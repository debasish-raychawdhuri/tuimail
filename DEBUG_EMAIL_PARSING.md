# Email Address Parsing Debug Guide

## Issue
Email addresses are still showing as "Unknown" in the email list despite implementing email address parsing.

## Debugging Steps

### 1. Enable Debug Output
To see what's happening during email parsing, set the `EMAIL_DEBUG` environment variable:

```bash
EMAIL_DEBUG=1 ./email_client
```

This will show:
- Email body length and first 200 characters
- Parsed email subjects and from addresses
- Header parsing details

### 2. Check Email Headers
The debug output will show if:
- Email bodies are being fetched correctly from IMAP
- Headers are being parsed by mail_parser
- From addresses are being extracted

### 3. Fallback Mechanisms
The code now includes multiple fallback mechanisms:
1. Try mail_parser's built-in header parsing
2. Parse headers manually from the headers map
3. Look for both "From" and "from" header variations
4. Show email address even if display name is missing

### 4. UI Improvements
The UI now shows:
- Display name if available
- Email address if no display name
- "Unknown" only if no address information at all

## Testing Email Address Parsing

The `parse_email_addresses()` function handles these formats:
- `"John Doe" <john@example.com>`
- `John Doe <john@example.com>`
- `<john@example.com>`
- `john@example.com`
- Multiple addresses: `John <john@example.com>, Jane <jane@example.com>`

## Potential Issues

1. **IMAP Fetch**: Email bodies might not include full headers
2. **Mail Parser**: The mail_parser crate might not be parsing headers correctly
3. **Header Names**: Some servers use different header capitalization
4. **Encoding**: Email headers might be encoded (RFC 2047)

## Next Steps

If addresses still show as "Unknown":
1. Run with `EMAIL_DEBUG=1` to see what's being parsed
2. Check if email bodies are being fetched correctly
3. Verify that headers contain "From" information
4. Consider using a different IMAP fetch command for headers only

## Manual Testing

You can test the parsing function independently:
```rust
let test_from = "John Doe <john@example.com>";
let addresses = parse_email_addresses(test_from);
// Should return: [EmailAddress { name: Some("John Doe"), address: "john@example.com" }]
```
