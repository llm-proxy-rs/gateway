pub fn common_styles() -> &'static str {
    r#"
        <style>
            table {
                border-collapse: collapse;
                margin: 20px 0 0 0;
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
        </style>
    "#
}

pub fn nav_menu() -> &'static str {
    r#"<br>
        <a href="/">Home</a>
        <a href="/generate-api-key">Generate API Key</a>
        <a href="/disable-api-keys">Disable API Keys</a>
        <a href="/view-usage-history">View Usage History</a>
        <a href="/update-usage-recording">Update Usage Recording</a>
        <a href="/clear-usage-history">Clear Usage History</a>
        <a href="/browse-models">Browse Models</a>
        <a href="/add-model">Add Model</a>
        <a href="/logout">Logout</a>
    "#
}
