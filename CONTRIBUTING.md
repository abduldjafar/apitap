# Contributing to ApiTap

Thank you for your interest in contributing to ApiTap! We're excited to have you join our community.

## 🌟 Ways to Contribute

- 🐛 **Report bugs** - Help us identify and fix issues
- 💡 **Suggest features** - Share your ideas for improvements
- 📝 **Improve documentation** - Help others understand the project better
- 🔧 **Submit pull requests** - Contribute code, tests, or fixes
- 📖 **Write tutorials** - Share your experience using ApiTap
- 💬 **Answer questions** - Help other users in discussions

## 🚀 Getting Started

### Prerequisites

- Rust toolchain (1.70 or higher)
- PostgreSQL 14+ (for development and testing)
- Git

### Development Setup

1. **Fork and clone the repository:**
   ```bash
   git clone https://github.com/YOUR_USERNAME/apitap.git
   cd apitap
   ```

2. **Set up your environment:**
   ```bash
   # Copy the example env file
   cp .env.example .env
   
   # Edit .env with your database credentials
   nano .env
   ```

3. **Build the project:**
   ```bash
   cargo build
   ```

4. **Run tests:**
   ```bash
   cargo test
   ```

5. **Run the example:**
   ```bash
   cargo run -- -m examples/sql -y examples/config/pipelines.yaml
   ```

## 📋 Development Workflow

### 1. Create a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/bug-description
```

Branch naming conventions:
- `feature/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation updates
- `refactor/` - Code refactoring
- `test/` - Test additions or updates

### 2. Make Your Changes

- Write clean, readable code
- Follow Rust naming conventions
- Add tests for new functionality
- Update documentation as needed
- Keep commits atomic and well-described

### 3. Code Quality Checks

Before submitting, ensure your code passes all checks:

```bash
# Format your code
cargo fmt

# Check for linting issues
cargo clippy -- -D warnings

# Run all tests
cargo test

# Build in release mode
cargo build --release
```

### 4. Commit Your Changes

Write clear, descriptive commit messages:

```bash
git add .
git commit -m "feat: add cursor-based pagination support"
```

**Commit message format:**
- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation changes
- `test:` - Test additions or changes
- `refactor:` - Code refactoring
- `perf:` - Performance improvements
- `chore:` - Maintenance tasks

### 5. Push and Create a Pull Request

```bash
git push origin feature/your-feature-name
```

Then create a pull request on GitHub with:
- Clear title describing the change
- Detailed description of what and why
- Link to related issues (if any)
- Screenshots (if UI-related)

## 🧪 Testing Guidelines

### Writing Tests

- **Unit tests**: Test individual functions/modules
- **Integration tests**: Test complete workflows
- **Doc tests**: Ensure examples in docs work

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_limit_offset() {
        let pagination = Pagination::LimitOffset {
            limit_param: "_limit".to_string(),
            offset_param: "_start".to_string(),
        };
        // Your test logic
        assert!(true);
    }
}
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_pagination_limit_offset

# Run tests with output
cargo test -- --nocapture

# Run tests in a specific file
cargo test --test integration_tests
```

## 📝 Code Style

### Rust Style Guidelines

We follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/):

1. **Naming Conventions:**
   - `snake_case` for functions, variables, modules
   - `PascalCase` for types, traits, enum variants
   - `SCREAMING_SNAKE_CASE` for constants

2. **Documentation:**
   ```rust
   /// Brief description of the function.
   ///
   /// More detailed explanation if needed.
   ///
   /// # Arguments
   ///
   /// * `param1` - Description of param1
   /// * `param2` - Description of param2
   ///
   /// # Returns
   ///
   /// Description of return value
   ///
   /// # Errors
   ///
   /// When and why errors might occur
   ///
   /// # Examples
   ///
   /// ```
   /// use apitap::example;
   /// let result = example::function(42);
   /// ```
   pub fn function(param1: i32, param2: &str) -> Result<String> {
       // Implementation
   }
   ```

3. **Error Handling:**
   - Use `Result<T, ApitapError>` for fallible operations
   - Provide context with error messages
   - Use `?` operator for propagation

4. **Async Code:**
   - Use `async/await` for async operations
   - Prefer `tokio::spawn` for concurrent tasks
   - Handle cancellation appropriately

### Formatting

We use `rustfmt` with default settings:

```bash
cargo fmt
```

### Linting

We use `clippy` with pedantic lints:

```bash
cargo clippy -- -D warnings
```

## 🐛 Reporting Bugs

### Before Submitting a Bug Report

1. Check if the bug is already reported in [Issues](https://github.com/abduldjafar/apitap/issues)
2. Try to reproduce with the latest version
3. Gather relevant information (logs, config, etc.)

### Bug Report Template

```markdown
**Describe the bug**
A clear description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Run command '...'
2. With config '...'
3. See error

**Expected behavior**
What you expected to happen.

**Actual behavior**
What actually happened.

**Environment:**
- OS: [e.g., Ubuntu 22.04]
- Rust version: [e.g., 1.70.0]
- ApiTap version: [e.g., 0.1.0]
- PostgreSQL version: [e.g., 17.0]

**Logs**
```
Paste relevant logs here
```

**Additional context**
Any other relevant information.
```

## 💡 Feature Requests

We love feature ideas! When suggesting a feature:

1. Check if it's already suggested
2. Explain the use case clearly
3. Describe the expected behavior
4. Consider implementation complexity
5. Be open to discussion

## 🔍 Code Review Process

### What We Look For

- **Correctness**: Does the code work as intended?
- **Tests**: Are there adequate tests?
- **Documentation**: Is the code well-documented?
- **Style**: Does it follow our style guide?
- **Performance**: Are there any performance concerns?
- **Security**: Are there any security implications?

### Review Timeline

- Initial review: Within 1-3 days
- Follow-up reviews: Within 1-2 days
- Merge: After approval from maintainers

## 🏗️ Project Structure

```
apitap/
├── .github/          # GitHub workflows and templates
├── examples/         # Example configurations and SQL
│   ├── config/       # Example YAML configs
│   └── sql/          # Example SQL modules
├── src/              # Source code
│   ├── cmd/          # CLI command handling
│   ├── config/       # Configuration parsing
│   ├── errors/       # Error types
│   ├── http/         # HTTP client and fetcher
│   ├── log/          # Logging setup
│   ├── pipeline/     # Pipeline orchestration
│   ├── utils/        # Utility functions
│   ├── writer/       # Database writers
│   ├── lib.rs        # Library entry point
│   └── main.rs       # Binary entry point
├── tests/            # Integration tests (to be added)
├── Cargo.toml        # Project manifest
└── README.md         # Project documentation
```

## 📚 Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [DataFusion Documentation](https://datafusion.apache.org/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Project README](./README.md)
- [Production Readiness Report](./PRODUCTION_READINESS_REPORT.md)

## 🎯 Areas Needing Contributions

### High Priority
- [ ] Comprehensive test suite
- [ ] Integration tests
- [ ] ClickHouse writer implementation
- [ ] BigQuery writer implementation
- [ ] OAuth2 authentication support

### Medium Priority
- [ ] Improved error messages
- [ ] Performance benchmarks
- [ ] Additional pagination modes
- [ ] Schema evolution handling
- [ ] Incremental sync state management

### Low Priority
- [ ] Web UI for monitoring
- [ ] Docker image
- [ ] Kubernetes manifests
- [ ] Additional documentation
- [ ] More examples

## 💬 Communication

- **Issues**: For bug reports and feature requests
- **Pull Requests**: For code contributions
- **Discussions**: For questions and general discussion

## 📜 License

By contributing to ApiTap, you agree that your contributions will be licensed under the MIT License.

## 🙏 Recognition

Contributors will be recognized in our README and release notes. Thank you for making ApiTap better!

---

**Questions?** Feel free to open a discussion or reach out to the maintainers.

**Happy Contributing! 🚀**
