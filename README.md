# bountui(/ˈbaʊn.ti/) - A Terminal UI for HashiCorp Boundary

**bountui** is a terminal-based user interface for interacting with [HashiCorp Boundary](https://www.hashicorp.com/products/boundary). It provides an intuitive way to navigate scopes, targets, and sessions, making Boundary management more accessible directly from your terminal.

---

## 🚀 Prerequisites

- **HashiCorp Boundary CLI**: Ensure `boundary` is installed and available in your system's `PATH`. You can find installation instructions in the [Boundary CLI documentation](https://developer.hashicorp.com/boundary/docs/cli).

---

## 🔒 Authentication

Currently, bountui has been tested with **OIDC authentication**. Other methods may work but are not guaranteed. Methods that require interaction with the terminal (e.g., password prompts) will **not** work.

### Authentication Compatibility Table

| Authentication Method   | Compatibility  |
|--------------------------|----------------|
| OIDC                    | ✅ Supported   |
| Password                | ❌ Not Supported |
| Auth Tokens             | ⚠️ Untested    |
| LDAP                    | ⚠️ Untested    |

---

## ⚙️ Compatibility

bountui has been tested with **Boundary 0.17.x**. Other versions may work but are not officially supported.

| Boundary Version | Compatibility |
|------------------|---------------|
| < 0.17.x         | ⚠️ Untested    |
| 0.17.x           | ✅ Supported   |
| 0.18.x           | ✅ Supported   |
| \> 0.18.x        | ⚠️ Untested    |

---

## 🛠️ Usage

bountui provides several keyboard shortcuts for interacting with Boundary resources:

| Shortcut | Function                                     |
|----------|----------------------------------------------|
| `/`      | Search within table views                    |
| `⏎`       | Show child elements (conext sensitive)       |
| `c`      | Connect to the selected target               |
| `Shift+c` | Show active sessions for the selected target |
| `Ctrl+d` | Stop the selected session                    |
| `Ctrl+c` | Quit bountui                                 |
| `Esc`    | Go back to the previous view                 |