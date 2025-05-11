use std::collections::HashMap;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyDict};
use serde_json::Value;

pub fn pyany_to_json_value(py_obj: &Py<PyAny>) -> PyResult<Value> {
    Python::with_gil(|py| {
        if py_obj.is_none(py) {
            return Ok(Value::Null);
        }
        
        if let Ok(val) = py_obj.extract::<bool>(py) {
            return Ok(Value::Bool(val));
        }
        
        if let Ok(val) = py_obj.extract::<i64>(py) {
            return Ok(Value::Number(val.into()));
        }
        
        if let Ok(val) = py_obj.extract::<f64>(py) {
            if let Some(number) = serde_json::Number::from_f64(val) {
                return Ok(Value::Number(number));
            }
            return Ok(Value::Null);
        }
        
        if let Ok(val) = py_obj.extract::<String>(py) {
            return Ok(Value::String(val));
        }
        
        if let Ok(val) = py_obj.extract::<Vec<PyObject>>(py) {
            let vec: Vec<Value> = val.into_iter()
                .map(|item| pyany_to_json_value(&item))
                .collect::<Result<_, _>>()?;
            return Ok(Value::Array(vec));
        }
            
        if let Ok(val) = py_obj.extract::<HashMap<String, PyObject>>(py) {
            let mut res_map = HashMap::new();
            for (key, value) in val {
                let key_str = key.to_string();
                let value_json = pyany_to_json_value(&value)?;
                res_map.insert(key_str, value_json);
            }
            let map: serde_json::Map<String, Value> = res_map.into_iter().collect(); 
            return Ok(Value::Object(map));
        }
       
        Err(PyErr::new::<PyTypeError, _>(
            format!("Cannot convert Python object to JSON: {:?}", py_obj)
        ))
    })
}

#[allow(dead_code)]
pub fn get_function_arg_names(py: Python, func: PyObject) -> PyResult<Vec<String>> {
    // Import the inspect module
    let inspect = py.import("inspect")?;
    
    // Call inspect.signature(func)
    let signature = inspect.call_method1("signature", (func,))?;
    
    // Get the parameters attribute
    let parameters = signature.getattr("parameters")?;
    
    // Convert to a dict and extract keys
    let dict = parameters.downcast::<PyDict>()?;
    let mut arg_names = Vec::new();
    
    for key in dict.keys() {
        arg_names.push(key.extract::<String>()?);
    }
    
    Ok(arg_names)
}