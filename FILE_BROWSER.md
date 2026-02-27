1# File Browser Feature

VinRouge now includes a **built-in file browser** for selecting CSV and Excel files!

## 🎯 How It Works

When adding CSV or Excel files, you'll see a graphical file browser instead of typing paths manually.

## 📂 Using the File Browser

### Quick Start

1. Launch VinRouge: `./target/release/vinrouge`
2. Press `1` - Add Data Source
3. Press `1` for CSV or `2` for Excel
4. **File Browser Opens!**

### Visual Interface

```
┌──────────────────────────────────────────────────────┐
│ Select File - /Users/you/Documents                   │
├──────────────────────────────────────────────────────┤
│  📁 ..                                               │
│  📁 data                                             │
│  📁 reports                                          │
│  📄 customers.csv (245.3 KB)                         │
│  📄 orders.csv (1.2 MB)                              │
│  📄 products.xlsx (89.5 KB)                          │
│                                                      │
│ ↑↓/jk: Navigate | Enter/→: Select | ←: Parent      │
└──────────────────────────────────────────────────────┘
```

## ⌨️ Keyboard Controls

### Navigation
| Key | Action |
|-----|--------|
| `↑` or `k` | Move up |
| `↓` or `j` | Move down |
| `Enter` or `→` or `l` | Open folder / Select file |
| `←` or `h` or `Backspace` | Go to parent folder |
| `Esc` | Cancel and go back |

### Features

**Visual Indicators:**
- 📁 **Folders** (shown in cyan)
- 📄 **Files** (shown in white)
- **Selected item** (shown in yellow/bold)
- **File sizes** displayed for files

**Smart Filtering:**
- **CSV Browser**: Shows only `.csv` files
- **Excel Browser**: Shows `.xlsx`, `.xls`, `.xlsm` files
- **Folders**: Always visible for navigation
- **Hidden files**: Automatically filtered out (except `..` for parent)

## 🎮 Workflows

### Example 1: Browse and Select CSV

1. Main Menu → Press `1`
2. Add Source → Press `1` (CSV)
3. **File browser opens**
4. Use `↑↓` to navigate through files
5. Press `Enter` on a folder to open it
6. Press `Enter` on a CSV file to select it
7. **File automatically added!**

### Example 2: Navigate Deep Directories

```
Start: /Users/you/Documents
  Press ↓ ↓ Enter     → Navigate to 'data' folder

Now in: /Users/you/Documents/data
  Press ↓ Enter       → Navigate to '2024' subfolder

Now in: /Users/you/Documents/data/2024
  Press ↓ ↓ ↓ Enter  → Select 'sales.xlsx'

✓ File added: /Users/you/Documents/data/2024/sales.xlsx
```

### Example 3: Quick Parent Navigation

```
Deep in folders but want to go back?

Press ←  → Go up one level
Press ←  → Go up another level
Press ←  → Keep going up

Or select '..' at the top and press Enter
```

## 💡 Tips & Tricks

### Fast Navigation
- **Vim Keys Work!** Use `hjkl` for navigation
- **Arrow Keys** also work for traditional navigation
- **Enter** on `..` goes to parent (same as `←`)

### File Selection
- Only matching files are shown (CSV browser only shows CSVs)
- Files show size for easy identification
- Current path is shown in the title bar

### Quick Actions
- **Want to go back?** Press `Esc` anytime
- **Wrong file?** Press `Esc` and try again
- **Folder mistake?** Press `←` to go back up

### Sorting
- Folders always appear first
- Files and folders sorted alphabetically
- Case-insensitive sorting

## 🎨 Visual Elements

**Colors:**
- **Cyan** 📁 - Directories
- **White** 📄 - Regular files
- **Yellow/Bold** - Selected item
- **Gray** - Help text at bottom

**Icons:**
- 📁 - Folder/Directory
- 📄 - File

**File Sizes:**
- Automatically formatted (KB, MB, GB)
- Only shown for files, not folders

## 🔍 Filtering Details

### CSV Browser
**Shows:**
- All folders (for navigation)
- Files ending in `.csv`

**Hides:**
- `.txt`, `.xlsx`, `.json`, etc.
- Hidden files (starting with `.`)
- Parent `..` is always shown

### Excel Browser
**Shows:**
- All folders (for navigation)
- Files ending in `.xlsx`, `.xls`, `.xlsm`

**Hides:**
- `.csv`, `.txt`, `.json`, etc.
- Hidden files (starting with `.`)
- Parent `..` is always shown

## 🐛 Troubleshooting

**Can't see my file:**
- Check if you're in the right folder
- Verify file has correct extension (.csv or .xlsx)
- Hidden files starting with `.` are filtered out

**Browser shows empty:**
- You might be in a folder with no matching files
- Press `←` to go up and try another folder
- Check file permissions

**Can't navigate:**
- Make sure you're not in read-only mode
- Try pressing `Esc` and reopening
- Check your terminal supports arrow keys

## 🆚 Manual Entry Still Available

Don't want to use the file browser? You can still manually type paths:

**For MSSQL databases:**
- The browser isn't used (connection strings are complex)
- You'll get the traditional text input

**To bypass browser** (future feature):
- Could add an option to toggle between browser and manual entry

## 🚀 Advanced Usage

### Quick Directory Jumping
1. Start browser
2. Navigate to commonly used folder
3. Select file there
4. Next time you add same file type, browser remembers location

### Multiple Files from Same Folder
1. Browse to folder once
2. Select first file → Added!
3. Add another source
4. Browse again → You're still in same folder
5. Select second file → Added!

---

**Enjoy the visual file browsing experience!** 📁✨

No more typing long paths - just navigate visually and select! 🎉
