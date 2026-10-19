//! Linear solvers for the symmetric positive definite network matrix.
//!
//! * Direct: sparse Cholesky (faer, AMD ordering). Robust and fast for typical sizes.
//! * Iterative: conjugate gradients preconditioned with an incomplete Cholesky
//!   factorization IC(0). Needs far less memory and converges much faster than a
//!   Jacobi preconditioner.

use std::sync::atomic::{AtomicBool, Ordering};

use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};
use faer::{Mat, Side};
use rayon::prelude::*;

use crate::error::{CrazeError, Result};

/// Compressed sparse row matrix (full symmetric pattern, sorted columns per row).
#[derive(Debug, Clone)]
pub struct Csr {
    pub n: usize,
    pub indptr: Vec<usize>,
    pub indices: Vec<u32>,
    pub vals: Vec<f64>,
}

impl Csr {
    /// Builds a CSR matrix from unsorted rows; duplicate entries are summed.
    pub fn from_rows(n: usize, rows: Vec<Vec<(u32, f64)>>) -> Self {
        let mut indptr = Vec::with_capacity(n + 1);
        let mut indices = Vec::new();
        let mut vals = Vec::new();
        indptr.push(0);
        for mut row in rows {
            row.sort_unstable_by_key(|e| e.0);
            let mut last: Option<u32> = None;
            for (c, v) in row {
                if last == Some(c) {
                    *vals.last_mut().unwrap() += v;
                } else {
                    indices.push(c);
                    vals.push(v);
                    last = Some(c);
                }
            }
            indptr.push(indices.len());
        }
        Csr { n, indptr, indices, vals }
    }

    pub fn nnz(&self) -> usize {
        self.vals.len()
    }

    pub fn spmv(&self, x: &[f64], y: &mut [f64]) {
        y.par_iter_mut().with_min_len(4096).enumerate().for_each(|(i, yi)| {
            let mut s = 0.0;
            for p in self.indptr[i]..self.indptr[i + 1] {
                s += self.vals[p] * x[self.indices[p] as usize];
            }
            *yi = s;
        });
    }

    pub fn residual_norm(&self, x: &[f64], b: &[f64]) -> f64 {
        let mut ax = vec![0.0; self.n];
        self.spmv(x, &mut ax);
        let r: f64 = ax.par_iter().zip(b).map(|(a, b)| (b - a) * (b - a)).sum();
        let nb: f64 = b.par_iter().map(|v| v * v).sum();
        (r / nb.max(1e-300)).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UsedSolver {
    Direct,
    Iterative,
}

#[derive(Debug, Clone)]
pub struct SolveStats {
    pub solver: UsedSolver,
    pub iterations: u32,
    pub relative_residual: f64,
}

/// Sparse Cholesky solve.
pub fn solve_direct(a: &Csr, b: &[f64]) -> Result<Vec<f64>> {
    let mut trip = Vec::with_capacity(a.nnz() / 2 + a.n);
    for i in 0..a.n {
        for p in a.indptr[i]..a.indptr[i + 1] {
            let j = a.indices[p] as usize;
            if j <= i {
                trip.push(Triplet::new(i, j, a.vals[p]));
            }
        }
    }
    let m = SparseColMat::<usize, f64>::try_new_from_triplets(a.n, a.n, &trip)
        .map_err(|e| CrazeError::Solver(format!("invalid matrix: {e:?}")))?;
    let llt = m
        .sp_cholesky(Side::Lower)
        .map_err(|e| CrazeError::Solver(format!("Cholesky factorization failed: {e:?}")))?;
    let rhs = Mat::<f64>::from_fn(a.n, 1, |i, _| b[i]);
    let x = llt.solve(&rhs);
    Ok((0..a.n).map(|i| x[(i, 0)]).collect())
}

/// Incomplete Cholesky factor L (lower triangle, CSR, diagonal stored last in each row).
struct Ic0 {
    indptr: Vec<usize>,
    indices: Vec<u32>,
    vals: Vec<f64>,
}

impl Ic0 {
    fn new(a: &Csr) -> Option<Ic0> {
        let n = a.n;
        let mut indptr = Vec::with_capacity(n + 1);
        let mut indices = Vec::new();
        let mut vals: Vec<f64> = Vec::new();
        indptr.push(0);
        let mut w = vec![0.0f64; n];
        for i in 0..n {
            let start = indices.len();
            let mut diag = 0.0;
            for p in a.indptr[i]..a.indptr[i + 1] {
                let k = a.indices[p] as usize;
                if k < i {
                    indices.push(k as u32);
                    vals.push(0.0);
                } else if k == i {
                    diag = a.vals[p];
                }
            }
            // Columns are sorted, so L[i,k] can be computed left to right.
            let row_end = indices.len();
            let mut q = a.indptr[i];
            for pos in start..row_end {
                let k = indices[pos] as usize;
                while a.indices[q] as usize != k {
                    q += 1;
                }
                let mut s = a.vals[q];
                // Row k of L: off-diagonals then diagonal last.
                let (ks, ke) = (indptr[k], indptr[k + 1] - 1);
                for r in ks..ke {
                    s -= vals[r] * w[indices[r] as usize];
                }
                let lkk = vals[ke];
                let lik = s / lkk;
                vals[pos] = lik;
                w[k] = lik;
                diag -= lik * lik;
            }
            for pos in start..row_end {
                w[indices[pos] as usize] = 0.0;
            }
            if diag <= 0.0 || !diag.is_finite() {
                return None;
            }
            indices.push(i as u32);
            vals.push(diag.sqrt());
            indptr.push(indices.len());
        }
        Some(Ic0 { indptr, indices, vals })
    }

    /// z = (L Lᵀ)⁻¹ r
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        let n = r.len();
        // Forward: L y = r
        for i in 0..n {
            let (s, e) = (self.indptr[i], self.indptr[i + 1] - 1);
            let mut v = r[i];
            for p in s..e {
                v -= self.vals[p] * z[self.indices[p] as usize];
            }
            z[i] = v / self.vals[e];
        }
        // Backward: Lᵀ z = y (column sweep over rows of L)
        for i in (0..n).rev() {
            let (s, e) = (self.indptr[i], self.indptr[i + 1] - 1);
            z[i] /= self.vals[e];
            let zi = z[i];
            for p in s..e {
                z[self.indices[p] as usize] -= self.vals[p] * zi;
            }
        }
    }
}

/// Preconditioned conjugate gradients.
pub fn solve_pcg(
    a: &Csr,
    b: &[f64],
    x0: Option<&[f64]>,
    tol: f64,
    max_iter: u32,
    cancel: &AtomicBool,
    progress: &(dyn Fn(u32, f64) + Sync),
) -> Result<(Vec<f64>, u32, f64)> {
    let n = a.n;
    let ic = Ic0::new(a);
    let diag_inv: Vec<f64> = (0..n)
        .map(|i| {
            let d = (a.indptr[i]..a.indptr[i + 1]).find(|&p| a.indices[p] as usize == i).map(|p| a.vals[p]).unwrap_or(1.0);
            if d.abs() > 1e-300 { 1.0 / d } else { 1.0 }
        })
        .collect();
    let precond = |r: &[f64], z: &mut [f64]| match &ic {
        Some(ic) => ic.apply(r, z),
        None => z.par_iter_mut().zip(r).zip(&diag_inv).for_each(|((z, r), d)| *z = r * d),
    };

    let mut x: Vec<f64> = x0.map(|v| v.to_vec()).unwrap_or_else(|| vec![0.5; n]);
    let mut r = vec![0.0; n];
    a.spmv(&x, &mut r);
    r.par_iter_mut().zip(b).for_each(|(ri, bi)| *ri = bi - *ri);
    let norm_b = b.par_iter().map(|v| v * v).sum::<f64>().sqrt().max(1e-300);
    let mut z = vec![0.0; n];
    precond(&r, &mut z);
    let mut p = z.clone();
    let mut rz: f64 = r.par_iter().zip(&z).map(|(a, b)| a * b).sum();
    let mut ap = vec![0.0; n];
    let mut rel = r.par_iter().map(|v| v * v).sum::<f64>().sqrt() / norm_b;
    let mut it = 0;
    while it < max_iter && rel > tol {
        if cancel.load(Ordering::Relaxed) {
            return Err(CrazeError::Cancelled);
        }
        a.spmv(&p, &mut ap);
        let pap: f64 = p.par_iter().zip(&ap).map(|(a, b)| a * b).sum();
        if pap <= 0.0 {
            break;
        }
        let alpha = rz / pap;
        x.par_iter_mut().zip(&p).for_each(|(x, p)| *x += alpha * p);
        r.par_iter_mut().zip(&ap).for_each(|(r, ap)| *r -= alpha * ap);
        rel = r.par_iter().map(|v| v * v).sum::<f64>().sqrt() / norm_b;
        precond(&r, &mut z);
        let rz_new: f64 = r.par_iter().zip(&z).map(|(a, b)| a * b).sum();
        let beta = rz_new / rz;
        rz = rz_new;
        p.par_iter_mut().zip(&z).for_each(|(p, z)| *p = z + beta * *p);
        it += 1;
        if it % 10 == 0 {
            progress(it, rel);
        }
    }
    progress(it, rel);
    if rel > tol * 100.0 {
        return Err(CrazeError::Solver(format!(
            "No convergence after {it} iterations (relative residual {rel:.2e}). Increase the iterations or use the direct solver."
        )));
    }
    Ok((x, it, rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1D Laplacian with Dirichlet ends.
    fn lap(n: usize) -> (Csr, Vec<f64>) {
        let rows = (0..n)
            .map(|i| {
                let mut r = vec![(i as u32, 2.0)];
                if i > 0 {
                    r.push((i as u32 - 1, -1.0));
                }
                if i + 1 < n {
                    r.push((i as u32 + 1, -1.0));
                }
                r
            })
            .collect();
        let mut b = vec![0.0; n];
        b[0] = 1.0;
        (Csr::from_rows(n, rows), b)
    }

    #[test]
    fn direct_and_pcg_agree() {
        let (a, b) = lap(200);
        let xd = solve_direct(&a, &b).unwrap();
        let cancel = AtomicBool::new(false);
        let (xi, _, _) = solve_pcg(&a, &b, None, 1e-12, 10_000, &cancel, &|_, _| {}).unwrap();
        for i in 0..200 {
            let exact = 1.0 - (i + 1) as f64 / 201.0;
            assert!((xd[i] - exact).abs() < 1e-10);
            assert!((xi[i] - exact).abs() < 1e-8);
        }
    }
}
