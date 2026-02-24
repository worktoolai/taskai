use serde_json::json;

use crate::db::connection;

pub fn run(json_output: bool) -> i32 {
    match connection::init_db() {
        Ok(path) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "success": true,
                        "data": { "path": path.to_string_lossy() }
                    }))
                    .unwrap()
                );
            } else {
                println!("Initialized taskai at {}", path.display());
            }
            0
        }
        Err(e) => {
            if json_output {
                println!("{}", serde_json::to_string_pretty(&crate::output::json::error(&e)).unwrap());
            } else {
                eprintln!("Error: {}", e.message);
            }
            1
        }
    }
}
