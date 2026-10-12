//! Development tool: exposes the CRAZE backend over HTTP (POST /invoke/<command>) so the
//! frontend can run in an ordinary browser for automated UI tests. Progress channels are
//! not forwarded. Never shipped.

use craze_app_lib::backend::{self, Backend, OptimizeArgs};
use craze_core::gds::LayerKey;
use craze_core::setup::{SimSettings, Setup};
use serde_json::{Value, json};
use tiny_http::{Header, Method, Response, Server};

fn arg<T: serde::de::DeserializeOwned>(v: &Value, k: &str) -> Result<T, String> {
    serde_json::from_value(v.get(k).cloned().unwrap_or(Value::Null)).map_err(|e| format!("{k}: {e}"))
}

enum Out {
    Json(Value),
    Bin(Vec<u8>),
}

fn handle(b: &Backend, prefs: &mut Value, cmd: &str, a: &Value) -> Result<Out, String> {
    let ok = |v: Value| Ok(Out::Json(v));
    match cmd {
        "open_layout" => ok(json!(b.open_layout(&arg::<String>(a, "path")?, arg(a, "topCell")?)?)),
        "layout_geometry" => Ok(Out::Bin(b.layout_geometry()?)),
        "run_simulation" => {
            let s: Setup = arg(a, "setup")?;
            let st: SimSettings = arg(a, "settings")?;
            ok(json!(b.simulate(&s, &st, &|_| {})?))
        }
        "result_fields" => Ok(Out::Bin(b.result_fields()?)),
        "cancel_job" => {
            b.cancel();
            ok(Value::Null)
        }
        "resolution_study" => {
            let s: Setup = arg(a, "setup")?;
            let st: SimSettings = arg(a, "settings")?;
            let r: Vec<u32> = arg(a, "resolutions")?;
            ok(json!(b.resolution_study(&s, &st, &r, &|_| {})?))
        }
        "run_optimization" => {
            let args: OptimizeArgs = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
            ok(json!(b.optimize(&args, &|_| {}, &|_| {})?))
        }
        "save_optimized_layout" => {
            let keys: Vec<LayerKey> = arg(a, "conductorKeys")?;
            b.save_optimized(&arg::<String>(a, "path")?, arg(a, "scale")?, keys)?;
            ok(Value::Null)
        }
        "material_library" => ok(json!(backend::material_library())),
        "list_examples" => ok(json!(backend::list_examples(backend::dev_examples_dir()))),
        "load_prefs" => ok(prefs.clone()),
        "save_prefs" => {
            *prefs = a.get("prefs").cloned().unwrap_or(Value::Null);
            ok(Value::Null)
        }
        "write_text_file" => {
            std::fs::write(arg::<String>(a, "path")?, arg::<String>(a, "contents")?).map_err(|e| e.to_string())?;
            ok(Value::Null)
        }
        "read_text_file" => ok(json!(std::fs::read_to_string(arg::<String>(a, "path")?).map_err(|e| e.to_string())?)),
        "write_base64_file" => ok(Value::Null),
        "startup_file" => ok(json!(std::env::var("CRAZE_TEST_FILE").ok())),
        other => Err(format!("unbekannter Befehl {other}")),
    }
}

fn main() {
    let port = std::env::var("CRAZE_BRIDGE_PORT").unwrap_or_else(|_| "1421".into());
    let server = Server::http(format!("127.0.0.1:{port}")).expect("port");
    eprintln!("craze-devbridge listening on http://127.0.0.1:{port}");
    let b = Backend::default();
    let mut prefs = Value::Null;
    let cors = |r: Response<std::io::Cursor<Vec<u8>>>| {
        r.with_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap())
            .with_header(Header::from_bytes("Access-Control-Allow-Headers", "*").unwrap())
    };
    for mut req in server.incoming_requests() {
        if *req.method() == Method::Options {
            let _ = req.respond(cors(Response::from_data(vec![])));
            continue;
        }
        let cmd = req.url().trim_start_matches("/invoke/").to_string();
        let mut body = String::new();
        let _ = req.as_reader().read_to_string(&mut body);
        let a: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
        let resp = match handle(&b, &mut prefs, &cmd, &a) {
            Ok(Out::Json(v)) => Response::from_data(v.to_string().into_bytes())
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()),
            Ok(Out::Bin(d)) => {
                Response::from_data(d).with_header(Header::from_bytes("Content-Type", "application/octet-stream").unwrap())
            }
            Err(e) => Response::from_data(json!(e).to_string().into_bytes())
                .with_status_code(400)
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()),
        };
        eprintln!("{cmd}");
        let _ = req.respond(cors(resp));
    }
}
