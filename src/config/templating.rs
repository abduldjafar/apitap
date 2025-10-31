use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::errors::Result;
use minijinja::path_loader;
use minijinja::value::{Kwargs, Value};
use minijinja::{Environment, Error as MjError};
use walkdir::WalkDir;

#[derive(Debug, Default, Clone)]
pub struct RenderCapture {
    pub sink: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct RenderedSql {
    pub name: String, 
    pub sql: String, 
    pub capture: RenderCapture,
}

pub fn build_env_with_captures(
    root: &str,
    shared_cap: &Arc<Mutex<RenderCapture>>,
) -> Environment<'static> {
    let mut env = Environment::new();
    env.set_loader(path_loader(root));

    // {{ sink(name="...") }}
    {
        let cap = Arc::clone(shared_cap);
        env.add_function(
            "sink",
            move |kwargs: Kwargs| -> std::result::Result<Value, MjError> {
                let name: String = kwargs.get("name")?;
                let mut c = cap.lock().unwrap();
                c.sink = name;
                Ok(Value::from(""))
            },
        );
    }

    // {{ use_source("...") }}
    {
        let cap = Arc::clone(shared_cap);
        env.add_function(
            "use_source",
            move |name: String| -> std::result::Result<Value, MjError> {
                let mut c = cap.lock().unwrap();
                c.source = name.clone();
                Ok(Value::from(name))
            },
        );
    }

    env
}

pub fn render_one(
    env: &Environment,
    shared_cap: &Arc<Mutex<RenderCapture>>,
    name: &str,
) -> Result<RenderedSql> {
    {
        let mut c = shared_cap.lock().unwrap();
        c.sink.clear();
        c.source.clear();
    }

    let tmpl = env.get_template(name)?;
    let sql = tmpl.render(())?;

    let capture = shared_cap.lock().unwrap().clone();
    Ok(RenderedSql {
        name: name.to_string(),
        sql,
        capture,
    })
}

pub fn list_sql_templates(root: impl AsRef<Path>) -> Result<Vec<String>> {
    let root = root.as_ref();
    let mut out = Vec::new();

    for entry_res in WalkDir::new(root) {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let is_sql = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("sql"))
            .unwrap_or(false);
        if !is_sql {
            continue;
        }

        let rel = match path.strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let name = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        out.push(name);
    }

    out.sort();
    Ok(out)
}
