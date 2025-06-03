use std::time::Duration;

pub const OUTPUT_FILENAME: &str = "project_structure.md";
pub const MARKDOWN_HEADER_CONTEXT: &str = "# Context";
pub const MARKDOWN_HEADER_STRUCTURE: &str = "## Project Structure";
pub const MARKDOWN_HEADER_FILES: &str = "## Files";
pub const MARKDOWN_CODE_BLOCK: &str = "```";

// Combined and reviewed from Python version and previous plans
pub const ADDITIONAL_IGNORE_PATTERNS: &[&str] = &[
    // Common VCS and build artifacts
    ".git/", ".hg/", ".svn/",
    "target/", "build/", "dist/", "pkg/", "node_modules/",
    // Python specific
    "__pycache__/", "*.pyc", "*.pyo", "*.pyd",
    ".env", ".venv", "venv/", "env/",
    "requirements.txt", // Often useful to see, but can be configured if user wants it ignored
    // Node specific
    "package-lock.json", "yarn.lock",
    // Common OS files
    ".DS_Store", "Thumbs.db",
    // Log files
    "*.log",
    // Temporary files
    "*.tmp", "*.swp", "*.swo",
    // Compiled outputs & binaries from various languages/tools
    "*.o", "*.so", "*.a", "*.dylib",
    "*.exe", "*.dll", "*.lib", "*.exp", "*.obj", "*.def",
    // Archives & compressed files
    "*.zip", "*.tar", "*.gz", "*.rar",
    // Image/Media (usually not context for code)
    "*.ico", "*.png", "*.jpg", "*.jpeg", "*.gif", "*.bmp", "*.tiff", "*.svg",
    "*.mp3", "*.mp4", "*.avi",
    // Database files
    "*.db", "*.sqlite", "*.sqlite3",
    // IDE specific
    ".idea/", ".vscode/", "*.sublime-project", "*.sublime-workspace",
];

pub const DEBOUNCE_DURATION: Duration = Duration::from_millis(750); // Slightly longer debounce
pub const UI_STATUS_MESSAGE_DURATION: Duration = Duration::from_secs(5); 