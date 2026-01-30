---
name: create-page
description: Guide for creating new UI pages in this Rust/Axum SSR application. Use when adding new routes, forms, or authenticated pages.
allowed-tools: Read, Edit, Write, Bash
---

# Create UI Pages in Gateway

This application uses **Axum with server-side rendering** - pages are Rust handler functions that return HTML strings. No frontend framework is used.

## 🎯 Naming Standards (CRITICAL)

This codebase follows **strict naming conventions**. You MUST follow these rules:

### Handler Functions
- **UI GET handlers:** `<action>_get` (e.g., `index_get`, `browse_models_get`, `view_profile_get`)
- **UI POST handlers:** `<action>_post` (e.g., `generate_api_key_post`, `disable_api_keys_post`)
- **API GET handlers:** `<action>_get_api` (e.g., `models_get_api`)
- **API POST handlers:** `<action>_post_api` (e.g., `chat_completions_post_api`, `v1_messages_post_api`)

### SQL Functions (in domain crates)
- Pattern: `<verb>_<entity>` (e.g., `create_user`, `get_models`, `check_api_key_exists`)
- Standard verbs: `create_`, `get_`, `check_`, `update_`, `delete_`, `disable_`, `enable_`
- ALWAYS use `sqlx::query!` or `sqlx::query_as!` macros (not raw `sqlx::query`)

### Route Paths
- Format: kebab-case `/action-entity`
- **Plural** for collection operations: `/disable-api-keys` (affects many)
- **Singular** for single-entity operations: `/add-model`, `/delete-model` (affects one)

### Struct Naming
- **Forms** (CSRF-protected web forms): `<Entity>Form` (e.g., `ApiKeyForm`, `AddModelForm`)
- **Responses** (API output): `<Entity>Response` (e.g., `UserResponse`, `ModelsResponse`)
- **Requests** (API input): `<Action>Request` (e.g., `CreateUsageRequest`)
- **Aggregated data**: Return tuples instead of creating structs (e.g., `(i64, i64)` for counts/totals)

## Quick Reference

All pages are defined in `server/src/main.rs` and follow these patterns:

### Pattern 1: Simple GET Page (Read-Only)

For pages that just display information without forms:

**IMPORTANT:** All UI route handlers MUST end with `_get` suffix (e.g., `your_page_get`)

```rust
async fn your_page_get(
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    // 1. Check authentication
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    // 2. Fetch data from domain modules if needed
    let data = your_module::get_data(&state.db_pool, &email).await?;

    // 3. Render HTML with common styles
    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Your Page</title>
            {}
        </head>
        <body>
            <div>
                <h1>Your Page</h1>
                <p>Content here: {}</p>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        data,
        nav_menu()
    );

    Ok(Html(html).into_response())
}
```

### Pattern 2: GET + POST Page (With Forms and CSRF)

For pages with forms that submit data:

**IMPORTANT:**
- GET handlers MUST end with `_get` suffix (e.g., `your_page_get`)
- POST handlers MUST end with `_post` suffix (e.g., `your_page_post`)

**Step 1: Define form struct**

```rust
#[derive(Deserialize)]
struct YourForm {
    authenticity_token: String,
    field1: String,
    field2: i32,
}
```

**Step 2: GET handler (displays form)**

```rust
async fn your_page_get(
    token: CsrfToken,
    session: Session,
) -> Result<Response, AppError> {
    // 1. Check authentication
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    // 2. Generate CSRF token
    let authenticity_token = get_authenticity_token(&token, &session).await?;

    // 3. Render form with CSRF token and common styles
    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Your Form</title>
            {}
        </head>
        <body>
            <div>
                <h1>Your Form</h1>
                <form method="post" action="/your-route">
                    <input type="hidden" name="authenticity_token" value="{}">
                    <label>Field 1: <input type="text" name="field1"></label><br>
                    <label>Field 2: <input type="number" name="field2"></label><br>
                    <button type="submit">Submit</button>
                </form>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        authenticity_token,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}
```

**Step 3: POST handler (processes form)**

```rust
async fn your_page_post(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
    form: Form<YourForm>,
) -> Result<Response, AppError> {
    // 1. Check authentication
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    // 2. Verify CSRF token
    verify_authenticity_token(&token, &session, &form.authenticity_token).await?;

    // 3. Process business logic via domain modules
    your_module::process_data(&state.db_pool, &email, &form.field1, form.field2).await?;

    // 4. Render success page with common styles
    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Success</title>
            {}
        </head>
        <body>
            <div>
                <h1>Success!</h1>
                <p>Your data was processed: {} {}</p>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        form.field1,
        form.field2,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}
```

## Step-by-Step: Adding a New Page

### 1. Create Handler Function(s)

Add your handler function(s) to `server/src/main.rs` (before the main function, around line 80-1000).

**Choose the right pattern:**
- Simple read-only page? → Use Pattern 1 (single GET handler)
- Form with submission? → Use Pattern 2 (GET + POST handlers)

### 2. Define Form Struct (if needed)

If your page has a form, define the struct near the top of `server/src/main.rs` (around lines 40-65):

```rust
#[derive(Deserialize)]
struct YourFormName {
    authenticity_token: String,  // Required for CSRF protection
    your_field1: String,
    your_field2: i32,
    // Match HTML form field names exactly
}
```

### 3. Register Route

Add your route to the main Router in the `main()` function (around lines 1210-1235):

**IMPORTANT:** Use `_get` and `_post` suffixed handler names

```rust
let app = Router::new()
    .route("/", get(index_get))
    // ... existing routes ...
    .route("/your-route", get(your_page_get).post(your_page_post))  // For forms
    // or
    .route("/your-route", get(your_page_get))  // For simple pages
    // ... rest of routes ...
```

### 4. Add Navigation Link

Update the `nav_menu()` function (around lines 88-100) to include your new page:

```rust
fn nav_menu() -> &'static str {
    r#"<br>
        <a href="/">Home</a>
        <!-- ... existing links ... -->
        <a href="/your-route">Your Page Title</a>
        <a href="/logout">Logout</a>
    "#
}
```

**Note:** You don't need to modify `common_styles()` unless you want to add new global styles.

### 5. Create Database Operations (if needed)

If your page needs database access, add functions to the appropriate domain module:

**For API key operations:** Edit `apikeys/src/lib.rs`
**For model operations:** Edit `models/src/lib.rs`
**For usage operations:** Edit `usage/src/lib.rs`
**For user operations:** Edit `users/src/lib.rs`

**IMPORTANT Naming Conventions:**
- Function names: `<verb>_<entity>` (e.g., `create_user`, `get_models`, `check_api_key_exists`)
- Standard verbs: `create_`, `get_`, `check_`, `update_`, `delete_`, `disable_`, `enable_`
- ALWAYS use `sqlx::query!` or `sqlx::query_as!` macros (not raw `sqlx::query`)
- For aggregated data (counts, totals, statistics): Return tuples instead of creating structs

Example in domain module:

```rust
// For simple data retrieval
pub async fn get_your_data(
    pool: &PgPool,
    user_email: &str,
) -> anyhow::Result<YourReturnType> {
    let result = sqlx::query_as!(
        YourReturnType,
        r#"
        SELECT field1, field2
        FROM your_table
        WHERE email = $1
        "#,
        user_email.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok(result)
}

// For aggregated data - return tuple instead of struct
pub async fn get_summary(
    pool: &PgPool,
    user_email: &str,
) -> anyhow::Result<(i64, i64)> {
    let result = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as "total!",
            COUNT(*) FILTER (WHERE is_active = true) as "active!"
        FROM your_table
        WHERE email = $1
        "#,
        user_email.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok((result.total, result.active))
}
```

## Important Conventions

### Common Styles
Every page MUST include `common_styles()` in the `<head>` section:

```rust
let html = format!(
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Page Title</title>
        {}
    </head>
    ...
    "#,
    common_styles()
);
```

The `common_styles()` function (defined at lines 69-86) provides:
- Table styling with borders and padding
- Light gray header backgrounds (#f2f2f2)
- Consistent table margins

**Only add inline styles** if you need page-specific styling beyond the common styles.

### Authentication
Every protected page MUST check for authentication:

```rust
let email = match session.get::<String>("email").await? {
    Some(email) => email,
    None => return Ok(Redirect::to("/login").into_response()),
};
```

### CSRF Protection
Forms MUST include CSRF protection:

1. **GET handler:** Generate token with `get_authenticity_token(&token, &session).await?`
2. **HTML form:** Include hidden field: `<input type="hidden" name="authenticity_token" value="{}">`
3. **POST handler:** Verify with `verify_authenticity_token(&token, &session, &form.authenticity_token).await?`
4. **Return value:** Use `(token, Html(html)).into_response()` instead of just `Html(html).into_response()`

### Navigation Menu
Every page should include `nav_menu()` at the bottom of the `<body>` within a `<div>` wrapper.

### HTML Structure
All pages should follow this consistent structure:

```rust
let html = format!(
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Page Title</title>
        {}
    </head>
    <body>
        <div>
            <!-- Page content here -->
            {}
        </div>
    </body>
    </html>
    "#,
    common_styles(),
    nav_menu()
);
```

### Error Handling
- All handlers return `Result<Response, AppError>`
- Use `?` for error propagation - errors auto-convert to HTTP responses
- Don't handle errors manually unless you need custom error messages

### HTML Rendering
- Use `format!` macro with raw string literals (`r#"..."#`)
- Keep HTML inline in the handler function
- Use `{}` placeholders for dynamic content
- Always include `common_styles()` in `<head>`
- Always include `nav_menu()` at the bottom of `<body>`
- Wrap content in a `<div>` tag
- Remember to HTML-escape user input if displaying untrusted data

### Database Access
- NEVER write SQL directly in handlers - extract to domain modules
- ALWAYS use domain modules (apikeys, models, users, usage)
- Pass `&state.db_pool` or `state.db_pool.as_ref()` to domain functions
- Use `sqlx::query!` or `sqlx::query_as!` macro for type-safe queries (NEVER use raw `sqlx::query`)
- Follow naming convention: `<verb>_<entity>` (e.g., `create_api_key`, `get_usage_records`)

### Handler Naming Convention
- **UI GET handlers:** Must end with `_get` (e.g., `index_get`, `browse_models_get`)
- **UI POST handlers:** Must end with `_post` (e.g., `generate_api_key_post`, `disable_api_keys_post`)
- **API GET handlers:** Must end with `_get_api` (e.g., `models_get_api`)
- **API POST handlers:** Must end with `_post_api` (e.g., `chat_completions_post_api`, `v1_messages_post_api`)

### Route Path Conventions
- Use kebab-case: `/generate-api-key`, `/browse-models`
- Use **plural** for collection operations: `/disable-api-keys` (affects many)
- Use **singular** for single-entity operations: `/add-model` (creates one), `/delete-model` (deletes one)

## Common Patterns

### Redirecting After POST

```rust
return Ok(Redirect::to("/success-page").into_response());
```

### Displaying Lists/Tables

Tables automatically receive styling from `common_styles()`:

```rust
let items = your_module::get_items(&state.db_pool, &email).await?;
let rows: String = items
    .iter()
    .map(|item| format!("<tr><td>{}</td><td>{}</td></tr>", item.field1, item.field2))
    .collect();

let html = format!(
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Items List</title>
        {}
    </head>
    <body>
        <div>
            <h1>Items</h1>
            <table>
                <thead><tr><th>Field 1</th><th>Field 2</th></tr></thead>
                <tbody>{}</tbody>
            </table>
            {}
        </div>
    </body>
    </html>
    "#,
    common_styles(),
    rows,
    nav_menu()
);
```

Tables will automatically have:
- Borders on cells
- 8px padding
- Light gray header backgrounds

### Handling Optional Form Fields

```rust
#[derive(Deserialize)]
struct YourForm {
    authenticity_token: String,
    required_field: String,
    optional_field: Option<String>,  // Use Option<T> for optional fields
}
```

## Example: Complete New Page

Here's a complete example of adding a "/view-profile" page:

**1. Add handler (with correct naming: `view_profile_get`):**

```rust
async fn view_profile_get(
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let user = users::get_user(&state.db_pool, &email).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Your Profile</title>
            {}
        </head>
        <body>
            <div>
                <h1>Your Profile</h1>
                <p><strong>Email:</strong> {}</p>
                <p><strong>Member since:</strong> {}</p>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        email,
        user.created_at,
        nav_menu()
    );

    Ok(Html(html).into_response())
}
```

**2. Register route (using `_get` suffix):**

```rust
let app = Router::new()
    // ... existing routes ...
    .route("/view-profile", get(view_profile_get))
    // ... rest of routes ...
```

**3. Add to navigation:**

```rust
fn nav_menu() -> &'static str {
    r#"<br>
        <a href="/">Home</a>
        <a href="/view-profile">View Profile</a>
        <!-- ... other links ... -->
    "#
}
```

**Note:** No custom styles needed - `common_styles()` provides all the basic styling.

## Testing Your Page

1. **Build and run:**
   ```bash
   cargo build
   cargo run
   ```

2. **Navigate to your page:**
   - Direct: `http://localhost:8000/your-route`
   - Or click the navigation link

3. **Test authentication:**
   - Try accessing without logging in first
   - Should redirect to `/login`

4. **Test forms (if applicable):**
   - Submit with valid data
   - Try submitting without CSRF token (should fail)
   - Test validation errors

## File Locations Quick Reference

| What | Where |
|------|-------|
| Page handlers | `server/src/main.rs` (lines 100-1100) |
| Form structs | `server/src/main.rs` (lines 320-990) |
| Common styles | `server/src/main.rs` `common_styles()` (lines 69-86) |
| Navigation menu | `server/src/main.rs` `nav_menu()` (lines 88-100) |
| Route registration | `server/src/main.rs` `main()` function (lines 1210-1235) |
| API key DB ops | `apikeys/src/lib.rs` |
| Model DB ops | `models/src/lib.rs` |
| Usage DB ops | `usage/src/lib.rs` |
| User DB ops | `users/src/lib.rs` |
| Error types | `myerrors/src/lib.rs` |
| Auth handlers | `myhandlers/src/lib.rs` (`login_get`, `logout_get`, `callback_get`) |

## Additional Resources

- **Axum Documentation**: https://docs.rs/axum/latest/axum/
- **SQLx Documentation**: https://docs.rs/sqlx/latest/sqlx/
- **Tower Sessions**: https://docs.rs/tower-sessions/latest/tower_sessions/

## When to Create New Domain Modules

If your page needs significant database operations that don't fit existing modules:

1. Create new workspace member: `mkdir your_module && cd your_module`
2. Run `cargo init --lib`
3. Add to root `Cargo.toml` workspace members
4. Define your database functions similar to existing modules
5. Add dependency in `server/Cargo.toml`
6. Import in `server/src/main.rs`: `use your_module;`
