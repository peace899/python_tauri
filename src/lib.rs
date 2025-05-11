use pyo3::prelude::*;
use pyo3::types::PyString;
use serde_json::Value;
use tauri::{image::Image, Emitter, Listener, Manager};
use std::{
    fs,
    path::PathBuf,
    str::FromStr,
    collections::HashMap,
    sync::Mutex,
};
use tauri::{AppHandle, Builder, WebviewWindowBuilder, Url};

mod python_utils;
use python_utils::pyany_to_json_value;

// Global state management
mod globals {
    use super::*;
    use once_cell::sync::Lazy;

    static APP_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));
    static FRONTEND_DIR: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
    static READY_CALLBACK: Lazy<Mutex<Option<Py<PyAny>>>> = Lazy::new(|| Mutex::new(None));
    static LISTENER_CALLBACK: Lazy<Mutex<Option<Py<PyAny>>>> = Lazy::new(|| Mutex::new(None));
    static PYCOMMANDS_HANDLER: Lazy<Mutex<HashMap<String, PyObject>>> = Lazy::new(|| Mutex::new(HashMap::new()));

    pub fn app_handle() -> Option<AppHandle> {
        APP_HANDLE.lock().unwrap().clone()
    }

    pub fn set_app_handle(handle: AppHandle) {
        *APP_HANDLE.lock().unwrap() = Some(handle);
    }

    pub fn frontend_dir() -> Option<String> {
        FRONTEND_DIR.lock().unwrap().clone()
    }

    pub fn set_frontend_dir(path: String) {
        *FRONTEND_DIR.lock().unwrap() = Some(path);
    }

    pub fn listener_callback() -> Option<Py<PyAny>> {
        Python::with_gil(|py| {
            LISTENER_CALLBACK.lock().unwrap().as_ref().map(|obj| obj.clone_ref(py))
        })
    }

    pub fn set_listener_callback(callback: Py<PyAny>) {
        * LISTENER_CALLBACK.lock().unwrap() = Some(callback);
    }


    pub fn ready_callback() -> Option<Py<PyAny>> {
        Python::with_gil(|py| {
            READY_CALLBACK.lock().unwrap().as_ref().map(|obj| obj.clone_ref(py))
        })
    }

    pub fn set_ready_callback(callback: Py<PyAny>) {
        *READY_CALLBACK.lock().unwrap() = Some(callback);
    }

    pub fn add_command_handler(key: String, value: PyObject) {
        PYCOMMANDS_HANDLER.lock().unwrap().insert(key, value);
    }

    pub fn get_command_handler(key: &str) -> Option<PyObject> {
        Python::with_gil(|py| {
            PYCOMMANDS_HANDLER.lock().unwrap()
                .get(key)
                .map(|py_any| py_any.clone_ref(py))
        })
    }
}

#[pyclass]
struct TauriApp {
    // Private field to prevent direct instantiation
    _private: (),
}

#[pymethods]
impl TauriApp {    
    #[staticmethod]
    fn on_ready(py: Python, callback: PyObject) -> PyResult<()> {
        globals::set_ready_callback(callback.clone_ref(py));
        Ok(())
    }

    #[staticmethod]
    fn mount_frontend(path: String) -> PyResult<()> {
        globals::set_frontend_dir(path);
        Ok(())
    }

    #[staticmethod]
    fn create_window(
        label: String,
        title: String,
        url: String,
        user_agent: Option<String>,
        width: Option<i32>,
        height: Option<i32>,
        maximized: bool,
        center: bool,
    ) -> PyResult<()> {
        let app_handle = globals::app_handle().ok_or_else(|| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("App handle not initialized"))?;
        
        let url = Url::from_str(&url)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid URL: {}", e)))?;

        let init_script = include_str!("./initialization_script.js");
        let init_script = init_script.replace("apptitle", &title.clone());
        
        let mut builder = WebviewWindowBuilder::new(
            &app_handle,
            label,
            tauri::WebviewUrl::External(url),
        )
        .title(title)
        .visible(true)
        .inner_size(width.unwrap_or(800) as f64, height.unwrap_or(600) as f64)
        .maximized(maximized)
        .initialization_script(init_script);

        if let Some(user_agent) = user_agent {
            builder = builder.user_agent(&user_agent);
        }

        if center {
            builder = builder.center();
        }
        
        builder.build()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Failed to build window: {}", e)))?;
        
        Ok(())
    }

    #[staticmethod]
    fn close() -> PyResult<()> {
        if let Some(app_handle) = globals::app_handle() {
            app_handle.exit(0);
        }
        Ok(())
    }

    #[staticmethod]
    fn emit(event_type: String, event_data: String) -> PyResult<()> {
        if let Some(app_handle) = globals::app_handle() {
            app_handle.emit(&event_type, event_data)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Failed to emit event: {}", e)))?;
        }
        Ok(())
    }

    #[staticmethod]
    fn listen(py: Python, callback: PyObject) -> PyResult<()> {
        globals::set_listener_callback(callback.clone_ref(py));
        Ok(())
    }

    #[staticmethod]
    fn register_commands(py: Python, handlers: Vec<PyObject>) -> PyResult<()> {       
        for handler in handlers {
            let name = handler.getattr(py, "__name__")?
                .extract::<String>(py)?;
            globals::add_command_handler(name, handler.clone_ref(py));
        }
        Ok(())             
    }   

    #[staticmethod]
    fn run(
        py: Python,
        identifier: String,
        product_name: String,
        icon_path: Option<String>,
        on_ready: Option<PyObject>,
    ) -> PyResult<i32> {
        // Set up Ctrl+C handler
        ctrlc::set_handler(move || {
            let _ = TauriApp::close();
        }).map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Failed to set Ctrl+C handler: {}", e)))?;

        // Configure Tauri context
        let mut context = tauri::generate_context!();
        let config = context.config_mut();
        
        config.identifier = identifier;
        config.product_name = Some(product_name);
        config.app.with_global_tauri = true;
        
        if let Some(callback) = on_ready {
            globals::set_ready_callback(callback.clone_ref(py));
        }
        
        // Run the Tauri application
        let result = Builder::default()            
            .plugin(tauri_plugin_dialog::init())
            .register_uri_scheme_protocol("fs", |_app, request| {handle_fs_protocol(&request)})
            .setup(|app| {setup_app(app, icon_path)})
            .invoke_handler(tauri::generate_handler![handle_py_command])
            .run(context);

        Ok(match result {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("Application error: {}", e);
                1
            }
        })
    }
}

// Helper functions
fn handle_fs_protocol(request: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    let front_dir = match globals::frontend_dir() {
        Some(dir) => dir,
        None => return not_found_response(),
    };

    let request_path = request.uri().path();
    let normalized_path = if request_path == "/" {
        "index.html"
    } else {
        request_path.trim_start_matches('/')
    };

    let path = PathBuf::from(&front_dir).join(normalized_path);
    
    if path.exists() {
        match fs::read(path) {
            Ok(content) => tauri::http::Response::builder()
                .status(200)
                .body(content)
                .unwrap(),
            Err(_) => not_found_response(),
        }
    } else {
        not_found_response()
    }
}

fn not_found_response() -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(404)
        .body(Vec::new())
        .unwrap()
}

fn setup_app(app: &mut tauri::App, icon_path: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    globals::set_app_handle(app.handle().clone());

    // Call the ready callback if it exists
    if let Some(callback) = globals::ready_callback() {
        Python::with_gil(|py| {
            callback.call0(py)?;
            Ok::<(), PyErr>(())
        })?;
    }

    // Set window icon if provided
    if let Some(icon_path) = &icon_path  {
        if let Some(window) = app.get_webview_window("main") {
            let icon = Image::from_path(PathBuf::from(icon_path))?;
            window.set_icon(icon)?;
        }
    }

    // Listen for webview events
    app.listen("webview_emit", |event| {
        log::debug!("Received event in Rust: {:?}", event);        
        
        // Get listen callback
        if let Some(callback) = globals::listener_callback() {            
            Python::with_gil(|py| {
                let args_py = PyString::new(py, &event.payload());
                if let Err(e) = callback.call1(py, (args_py,)) {
                    log::error!("Error in Python callback: {:?}", e);
                }
            });
        }
    });

    Ok(())
}

#[tauri::command]
fn handle_py_command(args: Value) -> Result<Option<Value>, String> {
    let command_name = args.get("command")
        .and_then(Value::as_str)
        .ok_or("Missing command name in args")?;
    
    let args_str = serde_json::to_string(&args)
        .map_err(|e| format!("Failed to serialize args: {}", e))?;

    let handler = globals::get_command_handler(command_name)
        .ok_or_else(|| format!("Command '{}' not registered", command_name))?;

    Python::with_gil(|py| {
        let args_py = PyString::new(py, &args_str);
        let result = handler.call1(py, (args_py,))
            .map_err(|e| format!("Python callback error: {}", e))?;
        
        pyany_to_json_value(&result)
            .map(Some)
            .map_err(|e| format!("Failed to convert Python result: {}", e))
    })
}

/// A Python module implemented in Rust.
#[pymodule]
fn python_tauri(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<TauriApp>()?;
    Ok(())
}