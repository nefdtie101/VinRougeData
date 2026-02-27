# VinRouge TUI Guide

Welcome to the **VinRouge Interactive Terminal UI**! This guide will help you get started.

## 🚀 Launching the TUI

Simply run:
```bash
./target/release/vinrouge
```

Or:
```bash
vinrouge interactive
```

## 📺 Screen Overview

### Main Menu
```
┌─────────────────────────────────────────────┐
│      VinRouge - Data Analysis Tool          │
└─────────────────────────────────────────────┘
┌─────────────────────────────────────────────┐
│              Main Menu                       │
│                                             │
│  1. Manage Data Sources                     │
│  2. Run Analysis                            │
│  3. View Results                            │
│  4. Reconcile Data                          │
│  5. Export Results                          │
│                                             │
│  ?. Help                                    │
│  q. Quit                                    │
└─────────────────────────────────────────────┘
┌─────────────────────────────────────────────┐
│ Ready                                       │
└─────────────────────────────────────────────┘
```

### Add Source Screen
```
┌─────────────────────────────────────────────┐
│         Select Data Source Type:            │
│                                             │
│  1. CSV File                                │
│  2. Excel File                              │
│  3. MSSQL Database                          │
│                                             │
│  Esc. Back to Main Menu                    │
└─────────────────────────────────────────────┘
```

### Input Screen
```
┌─────────────────────────────────────────────┐
│ Enter CSV file path:                        │
└─────────────────────────────────────────────┘
┌─────────────────────────────────────────────┐
│ /path/to/your/data.csv_                     │
└─────────────────────────────────────────────┘

Press Enter to add, Esc to cancel
```

### Source List
```
┌─────────────────────────────────────────────┐
│      Configured Sources                     │
│                                             │
│  CSV: /data/customers.csv                   │
│  Excel: /data/sales.xlsx                    │
│  MSSQL: Production Database                 │
│                                             │
│  ↑↓: Navigate | d: Delete | Esc: Back       │
└─────────────────────────────────────────────┘
```

### Results Viewer
```
┌─────────────────────────────────────────────┐
│          Analysis Results                   │
│                                             │
│  ═══ ANALYSIS RESULTS ═══                   │
│                                             │
│  Tables: 5                                  │
│  Relationships: 12                          │
│  Workflows: 3                               │
│                                             │
│  ─── TABLES ───                             │
│                                             │
│  • customers (8 columns)                    │
│    Source: csv                              │
│    Rows: 1,250                              │
│                                             │
│  • orders (6 columns)                       │
│    Source: csv                              │
│    Rows: 3,400                              │
│                                             │
│  ↑↓: Scroll | Esc: Back                     │
└─────────────────────────────────────────────┘
```

## 🎮 Workflow Examples

### Example 1: Analyzing a CSV File

1. **Launch**: `vinrouge`
2. **Press `1`**: Manage Data Sources
3. **Press `1`**: CSV File
4. **Type path**: `/path/to/data.csv`
5. **Press Enter**: Source added
6. **Press Esc**: Back to main menu
7. **Press `2`**: Run Analysis
8. *(Wait for analysis)*
9. **Automatic**: Results displayed
10. **Use ↑↓**: Scroll through results
11. **Press `q`**: Quit

### Example 2: Comparing Multiple Sources

1. Launch TUI
2. Add CSV source (press `1`, `1`)
3. Add Excel source (press `1`, `2`)
4. Add MSSQL source (press `1`, `3`)
5. View sources (press `1`)
6. Run analysis (press `2`)
7. View relationships between all sources
8. Export results if needed

### Example 3: Managing Sources

1. Launch TUI
2. Press `1` - Manage Sources
3. Use ↑↓ to select a source
4. Press `d` to delete
5. Press Esc to return
6. Add new sources as needed

## ⌨️ Complete Keyboard Reference

### Global Keys (Work Everywhere)
| Key | Action |
|-----|--------|
| `q` | Quit application |
| `Ctrl+C` | Quit application |
| `Esc` | Go back / Cancel |
| `?` or `F1` | Show help screen |

### Main Menu
| Key | Action |
|-----|--------|
| `1` | Manage data sources |
| `2` | Run analysis |
| `3` | View results |
| `4` | Reconcile data |
| `5` | Export results |

### Add Source Menu
| Key | Action |
|-----|--------|
| `1` | Add CSV file |
| `2` | Add Excel file |
| `3` | Add MSSQL database |
| `Esc` | Back to main menu |

### Input Fields
| Key | Action |
|-----|--------|
| Letters/Numbers | Type characters |
| `Backspace` | Delete last character |
| `Enter` | Confirm input |
| `Esc` | Cancel |

### Source List
| Key | Action |
|-----|--------|
| `↑` or `k` | Move selection up |
| `↓` or `j` | Move selection down |
| `d` | Delete selected source |
| `Esc` | Back to main menu |

### Results Viewer
| Key | Action |
|-----|--------|
| `↑` or `k` | Scroll up |
| `↓` or `j` | Scroll down |
| `PageUp` | Scroll up one page |
| `PageDown` | Scroll down one page |
| `Home` | Jump to top |
| `Esc` | Back to main menu |

## 💡 Tips & Tricks

### Fast Navigation
- Use **number keys (1-5)** in the main menu for quick access
- Use **vim-style keys (j/k)** for quick scrolling if you prefer
- **PageUp/PageDown** work great for long results

### Error Handling
- If a source fails to load, you'll see an error in the status bar
- Press **Esc** to return to the previous screen
- Delete problematic sources with **d** in the source list

### Multiple Sources
- Add as many sources as you need before running analysis
- The analyzer will find relationships across ALL sources
- View all sources in one list (press `1`)

### Results Navigation
- Results can be very long - use **Home** to jump back to top
- Scroll speed is one line at a time for precision
- **PageUp/PageDown** for faster navigation

## 🎨 Visual Elements

The TUI uses colors to enhance readability:

- **Cyan**: Titles and important headers
- **Yellow**: Selected items and section headers
- **Green**: Positive indicators (success, counts)
- **White**: Normal text
- **Gray**: Help text and secondary info
- **Blue/Magenta**: Relationship indicators
- **Red**: Errors (when they occur)

## 🆘 Getting Help

- Press **?** or **F1** anywhere to see the help screen
- The status bar shows current status and hints
- Each screen shows relevant keyboard shortcuts at the bottom

## 🐛 Troubleshooting

**TUI won't launch:**
- Make sure your terminal supports ANSI colors
- Try a different terminal (iTerm2, Terminal.app, etc.)

**Can't see cursor:**
- The cursor is hidden by design in menu mode
- It appears when typing in input fields

**Keyboard shortcuts not working:**
- Make sure you're in the right screen mode
- Check if another program is capturing the keys

**Analysis hangs:**
- Check your database connection
- Verify file paths are correct
- Look at the status bar for hints

---

Enjoy using VinRouge! 🍷
