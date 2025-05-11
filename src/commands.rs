#[tauri::command]
fn command(args: Value) -> Result<Option<Value>, String> {
    let args_str = serde_json::to_string(&args).unwrap();
    
    if let Some(callback) = IPC_CALLBACK.lock().unwrap().as_ref() {
        Ok(Some(Python::with_gil(|py| {
            let args_py = PyString::new(py, &args_str);
            let result = callback.call1(py, (args_py,)).unwrap();
            let result_value = pyany_to_json_value(&result).unwrap_or_default();
        
            Some(result_value)
           
        }).unwrap()))
    } else {
        Err("Please register command callback using on_command()".into())
    }
}

static IPC_CALLBACK: Mutex<Option<Py<PyAny>>> = Mutex::new(None);

#[staticmethod]
    fn on_command(py: Python, callback: PyObject) -> PyResult<()> {
        // Store the Python callback
        let callback_ref = callback.clone_ref(py);
        IPC_CALLBACK.lock().unwrap().replace(callback_ref);
        Ok(())
    }