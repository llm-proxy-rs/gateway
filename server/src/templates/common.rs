pub fn common_styles() -> &'static str {
    r#"
        <style>
            table {
                border-collapse: collapse;
                margin: 20px 0 0 0;
                width: 100%;
            }
            th, td {
                border: 1px solid #ddd;
                padding: 8px;
                text-align: left;
            }
            th {
                background-color: #f2f2f2;
            }
            tr:nth-child(even) {
                background-color: #f9f9f9;
            }
            input[type="text"] {
                width: 400px;
            }
            form {
                margin: 0;
            }
        </style>
    "#
}

pub fn nav_menu() -> &'static str {
    r#"<br>
        <a href="/">Home</a>
        <a href="/generate-api-key">Generate API Key</a>
        <a href="/disable-api-keys">Disable API Keys</a>
        <a href="/browse-models">Browse Models</a>
        <a href="/add-model">Add Model</a>
        <a href="/logout">Logout</a>
    "#
}
