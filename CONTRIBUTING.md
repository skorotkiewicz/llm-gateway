# Contributing to LLM Gateway

Thank you for your interest in contributing to **LLM Gateway**! We welcome all kinds of contributions—from bug reports and documentation improvements to new features and security hardening.

## How to Contribute

### 1. Report Bugs
If you find a bug, please open an issue on GitHub. Include:
- Your Linux kernel version (`uname -r`).
- Steps to reproduce the issue.
- Expected vs. actual behavior.

### 2. Suggest Features
Have an idea to make LLM Gateway better? Open an issue with the "feature request" tag. We are particularly interested in:
- OpenAI spec compatibility.
- Anthropic spec compatibility.
- Ollama spec compatibility.

### 3. Submit Pull Requests
1. **Fork** the repository.
2. **Create a branch** for your feature (`git checkout -b feature/cool-new-thing`).
3. **Write Tests:** If you add a new utility or feature, please include unit or integration tests.
4. **Run Checks:** Ensure the code passes linting and formatting:
   ```bash
   cargo fmt --check
   cargo clippy
   cargo test
   ```
5. **Commit & Push:** Use descriptive commit messages.
6. **Open a PR:** Describe your changes clearly in the PR template.

## Technical Guidelines
- **Rust Edition:** We use Rust 2021.
- **Safety:** Minimize `unsafe` blocks. If you use `unsafe`, please provide a `// SAFETY:` comment explaining why it is necessary and correct.
- **Minimalism:** LLM Gateway aims to stay small. Avoid adding heavy dependencies unless absolutely necessary.

## License
By contributing to LLM Gateway, you agree that your contributions will be licensed under the project's **MIT OR Apache-2.0** license.
