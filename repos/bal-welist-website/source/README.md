# Server Status: Complete Explanation and Reference Guide

This page explains all the **Will Executor server health statuses** used throughout the BAL Web Interface. Each status type provides critical information about server operation, health checks, and potential issues for administrators monitoring the Bitcoin After Life network.

## Status Code Overview

| Status | Description | When It Occurs |
|--------|-------------|----------------|
| `OK` | Health check passed | Server is online, functioning normally |
| `TIMEOUT: <message>` | Connection timeout | Server response took too long |
| `CONNECTION: <message>` | Connection error | Network connectivity issues |
| `STATUS: <code>` | HTTP error (from server) | Server returned non-200 status |
| `WRONG REPLY: <message>` | Invalid JSON response | Server response couldn't be parsed |
| `REQUEST: <message>` | Request error | Client-side request failed |
| `BODY: <message>` | Body read error | Failed to read response body |
| `DECODE: <message>` | Decode error | Failed to decode response content |
| `KO` | Generic error | Unknown or unspecified error |

## Status Types Explained

### 🟢 Good Statuses
**`OK`** - Everything is functioning correctly:
- Server is online and responding
- Health check completed successfully
- Ready for new registration/processing

### 🟡 Warning/Moderate Issues
**`TIMEOUT: <message>`** - Connection timed out:
- Server took too than expected to respond
- Could indicate server overload or network issues
- Requires attention but server may still be functional

**`CONNECTION: <message>`** - Connection error:
- Network connectivity problems
- Server might be reachable but cannot communicate
- Potential infrastructure or configuration issue

**`STATUS: <code>`** - HTTP error:
- Server responded with an error code (4xx or 5xx)
- Could indicate misconfiguration, maintenance, or issues |
- Different error codes mean different things

### 🔴 Critical Issues
**`WRONG REPLY`** - Invalid JSON response:
- Server response format is incorrect
- May indicate server software bug or corruption
- Prevents proper data processing

**`REQUEST`** - Request error:
- Client request failed
- Network or protocol issue
- Server may be unreachable

**`BODY`** - Body read error:
- Failed to read response body after successful connection
- Could indicate server processing issues
- Data corruption or transmission problems

**`DECODE`** - Decode error:
- Failed to decode response content
- Encoding mismatch or corrupted data
- Prevents proper data interpretation

**`KO`** - Generic error:
- Unknown or unspecified error
- Fallback for all other unidentified issues
- Indicates something went wrong but exact cause unknown

## Example Status Messages

### Normal Operations
```
Status: OK
Meaning: Health check passed
Display: Server is healthy and ready
```

### Network/Time Issues
```
Status: TIMEOUT: Connection timed out after 30 seconds
Meaning: Server took too long to respond
Display: "Connection timeout (Connection timed out after 30 seconds)"
```

### HTTP Errors
```
Status: STATUS: 500
Meaning: Server returned HTTP 500 Internal Server Error
Display: "HTTP error (HTTP error)"
```

### Response Processing Issues
```
Status: WRONG REPLY: Invalid JSON
Meaning: Server response could not be parsed as JSON
Display: "Invalid JSON response (Invalid JSON)"
```

## Practical Usage Guide

### For Server Administrators
- **OK**: Check server resources and logs if needed
- **TIMEOUT/CONNECTION**: Check server network connectivity
- **STATUS**: Review server logs and restart if needed
- **WRONG REPLY/DECODE/REQUEST**: Investigate server software issues
- **KO**: Review server errors and restart

### For Users
- **OK**: Server is working correctly
- **TIMEOUT/CONNECTION**: Temporary issue, try again later
- **STATUS**: Server may be down for maintenance
- **Other errors**: Server issue, contact support if persistent

## Monitoring Tips

1. **Continuous Monitoring**: Regular checks of status codes help detect issues early
2. **Alert Setup**: Configure alerts for non-OK statuses
3. **Log Analysis**: Correlate status codes with server logs
4. **Capacity Planning**: Address persistent timeouts/connections with infrastructure improvements

## Status Processing Logic

### Status Code Processing
```javascript
const statusMap = {
  'OK': 'Health check passed',
  'TIMEOUT': 'Connection timeout',
  'CONNECTION': 'Connection error',
  'STATUS': 'HTTP error',
  'WRONG REPLY': 'Invalid JSON response',
  'REQUEST': 'Request error',
  'BODY': 'Body read error',
  'DECODE': 'Decode error',
  'KO': 'Generic error'
};

// Extract base status from status message
const st = (data.status||'').split(':')[0];

// Display only the description (e.g., "Connection timeout" instead of "TIMEOUT: Connection timeout")
statusDisplay = statusMap[st] || '—';
```

## Related Documentation

For more details on server setup, monitoring, and troubleshooting, refer to:
- [Server Administration Guide](./administration)
- [Status Monitoring Setup](./monitoring-setup)
- [Error Resolution Handbook](./error-resolve)

## Additional Information

This status system provides a clear, human-readable way to understand server health at a glance, making it easier for administrators to quickly identify and respond to issues.