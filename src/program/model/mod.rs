//! The list the person looks at, made of what Linear sent: filtered, grouped by the kind of the
//! state and sorted. It lives in the program, not in the page. It knows nothing of the page; it
//! takes and gives plain Rust values.

pub mod filter;
pub mod groups;
pub mod projects;
pub mod time;
