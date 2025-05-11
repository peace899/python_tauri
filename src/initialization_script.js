window.invoke = async (command, args) => {
    args.command = command;
    let response = await window.__TAURI__.core.invoke('handle_py_command', {args: args});
    return response
};

window.listen = window.__TAURI__.event.listen;            

window.emit = (message) => window.__TAURI__.event.emit('webview_emit', {message: message});
window.alert = (text) => window.__TAURI__.dialog.message(text, {type: "info", title: "apptitle"})

