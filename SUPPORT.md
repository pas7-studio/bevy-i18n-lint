# Support

## Getting Help

### Documentation
- Read the [README.md](README.md) for installation and usage instructions
- Check the [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines
- Review the [AGENTS.md](AGENTS.md) for AI agent guidelines

### Community Support
- **GitHub Issues**: For bug reports and feature requests
  - Use the provided issue templates
  - Search existing issues before creating new ones
- **GitHub Discussions**: For general questions and community discussion
  - Ask questions about usage
  - Share your use cases
  - Connect with other users

## Troubleshooting

### Common Issues

#### Installation Problems
```bash
# Ensure you have Rust installed
rustc --version
cargo --version

# Clean and rebuild
cargo clean
cargo install --path .
```

#### CLI Not Working
```bash
# Check help output
bevy-i18n-lint --help

# Run with verbose output
bevy-i18n-lint --verbose
```

#### JSON Output Issues
- Ensure you're using the correct flags
- Check that output is valid JSON using `jq` or similar tools

### Need More Help?

If you can't find a solution:
1. Check existing GitHub issues and discussions
2. Create a new issue with detailed information:
   - Version of bevy-i18n-lint
   - Operating system
   - Command you're running
   - Full error message or output
   - Steps to reproduce the problem
   - Expected vs actual behavior

## Professional Support

For enterprise or commercial support options, please contact the maintainers through GitHub discussions.

## Contributing

Found a bug? Have a feature request? We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for details.