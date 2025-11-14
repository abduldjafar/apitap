# Security Policy

## Supported Versions

We take security seriously and aim to address vulnerabilities promptly.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in ApiTap, please follow these steps:

### 🔒 Private Disclosure

**DO NOT** open a public GitHub issue for security vulnerabilities.

Instead, please report security issues privately by:

1. **Email**: Send details to the maintainers (create a private security advisory on GitHub)
2. **GitHub Security Advisory**: Use GitHub's [private vulnerability reporting](https://github.com/abduldjafar/apitap/security/advisories/new)

### 📝 What to Include

Please include the following information in your report:

- **Description**: A clear description of the vulnerability
- **Impact**: What could an attacker do with this vulnerability?
- **Reproduction Steps**: Detailed steps to reproduce the issue
- **Affected Versions**: Which versions are affected?
- **Proof of Concept**: Code or configuration demonstrating the issue (if applicable)
- **Suggested Fix**: If you have ideas on how to fix it (optional)

### Example Report

```markdown
**Vulnerability Type**: SQL Injection / Authentication Bypass / etc.

**Description**:
A clear description of what the vulnerability is.

**Impact**:
- Unauthorized access to database
- Data exfiltration
- Remote code execution
- etc.

**Affected Versions**:
- 0.1.0 and earlier

**Reproduction Steps**:
1. Configure ApiTap with...
2. Send a request to...
3. Observe that...

**Proof of Concept**:
```yaml
# Example malicious config
sources:
  - name: malicious_source
    url: "http://evil.com/'; DROP TABLE users;--"
```

**Suggested Fix**:
Use parameterized queries instead of string concatenation.
```

### ⏱️ Response Timeline

- **Initial Response**: Within 48 hours
- **Confirmation**: Within 7 days
- **Fix Development**: Depends on severity
  - Critical: 1-7 days
  - High: 7-14 days
  - Medium: 14-30 days
  - Low: 30-90 days
- **Public Disclosure**: After fix is released

### 🏆 Recognition

We appreciate security researchers who help keep ApiTap secure:

- Your name will be listed in the security advisory (with your permission)
- Credit in the CHANGELOG
- Public thanks in release notes

## Security Best Practices

When using ApiTap in production:

### 1. Environment Variables

✅ **DO**: Use environment variables for sensitive credentials

```yaml
targets:
  - name: postgres_sink
    type: postgres
    auth:
      username_env: POSTGRES_USER
      password_env: POSTGRES_PASSWORD
```

❌ **DON'T**: Hardcode credentials in YAML files

```yaml
# ❌ Avoid this in production!
targets:
  - name: postgres_sink
    type: postgres
    auth:
      username: postgres
      password: secretpassword123
```

### 2. File Permissions

Protect your configuration files:

```bash
# Restrict access to config files
chmod 600 .env
chmod 600 pipelines.yaml

# Ensure only the application user can read them
chown apitap:apitap .env pipelines.yaml
```

### 3. Network Security

- Use TLS/SSL for database connections
- Run ApiTap in a private network when possible
- Use firewall rules to restrict access
- Implement network isolation

### 4. Database Security

```yaml
# Use SSL connections for PostgreSQL
targets:
  - name: postgres_sink
    type: postgres
    host: db.example.com
    port: 5432
    # Ensure your PostgreSQL server requires SSL
```

### 5. Input Validation

- Validate all API URLs before use
- Sanitize table names and column names
- Be cautious with user-provided SQL or templates
- Use parameterized queries (ApiTap does this by default)

### 6. Logging

- Avoid logging sensitive data
- Use `--log-json` for production logging
- Implement log rotation
- Monitor logs for suspicious activity
- Don't log passwords or tokens

### 7. Updates

- Keep ApiTap updated to the latest version
- Subscribe to security advisories
- Regularly update dependencies with `cargo update`
- Monitor for Dependabot alerts

### 8. Least Privilege

- Run ApiTap with minimal required permissions
- Use database users with restricted privileges
- Limit API access tokens to required scopes
- Don't run as root

### 9. Secrets Management

Consider using dedicated secrets management solutions:

- HashiCorp Vault
- AWS Secrets Manager
- Azure Key Vault
- Kubernetes Secrets
- Environment variable management tools

### 10. Monitoring

- Monitor for unusual activity
- Set up alerts for failures
- Track resource usage
- Log all ETL operations
- Regular security audits

## Known Security Considerations

### SQL Injection

ApiTap uses DataFusion's query engine which uses **parameterized queries** and **prepared statements** where applicable. However:

- Custom SQL in modules is executed as-is
- Ensure you trust the source of SQL files
- Validate table and column names from external sources

### Credential Storage

- Credentials in .env files are stored in plaintext
- Use appropriate file permissions (chmod 600)
- Consider using encrypted secret management solutions
- Never commit .env files to version control

### HTTP Requests

- All HTTP requests are made to URLs in your configuration
- Validate API URLs to prevent SSRF (Server-Side Request Forgery)
- Use HTTPS for sensitive data
- Implement rate limiting on API endpoints

### Dependencies

We monitor our dependencies for vulnerabilities:

```bash
# Check for known vulnerabilities
cargo audit

# Update dependencies
cargo update
```

## Security Checklist

Before deploying to production:

- [ ] All credentials use environment variables
- [ ] .env file has restricted permissions (600)
- [ ] Configuration files are secured
- [ ] Database connections use SSL/TLS
- [ ] Application runs with least privilege
- [ ] Logs don't contain sensitive data
- [ ] Latest version of ApiTap is used
- [ ] Dependencies are up to date
- [ ] Monitoring and alerting are configured
- [ ] Backup and recovery plans are in place

## Responsible Disclosure

We follow responsible disclosure practices:

1. **Private Reporting**: Vulnerabilities reported privately
2. **Collaboration**: Work with reporter to understand and fix
3. **Fix Development**: Develop and test a patch
4. **Release**: Release fixed version
5. **Disclosure**: Public disclosure with credit to reporter
6. **Timeline**: Typically 90 days from report to disclosure

## Contact

For security concerns, please use:

- GitHub Security Advisories (preferred)
- Create a private security advisory at: https://github.com/abduldjafar/apitap/security/advisories/new

For general questions about security practices, you can open a public discussion.

## Acknowledgments

We thank the following security researchers for their contributions:

- (None yet - you could be first!)

---

**Last Updated**: 2025-11-14
