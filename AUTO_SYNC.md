# Automatic Email Synchronization

## Overview

The email client now automatically downloads new emails when you navigate between folders and provides real-time sync status feedback.

## 🔄 **Auto-Sync Triggers**

### **1. Folder Navigation**
- **Up/Down arrows in folder list**: Automatically syncs when switching folders
- **Enter to select folder**: Syncs the selected folder
- **Real-time feedback**: Shows "Syncing..." status during the process

### **2. Manual Refresh**
- **Press 'r' key**: Manually refresh current folder
- **Immediate sync**: Downloads any new emails from server
- **Status update**: Shows sync completion time and email count

### **3. Application Startup**
- **Initial load**: Syncs the default folder (INBOX) on startup
- **Cache loading**: Shows cached emails immediately, then syncs for updates

## 📊 **Sync Status Display**

### **Status Bar Information**
The bottom status bar now shows:
- **Current folder**: Which folder you're viewing
- **Email count**: Number of emails in current folder
- **Sync status**: 
  - `Syncing...` during active sync
  - `Last sync: HH:MM:SS` showing when last synced
- **Context help**: Relevant keyboard shortcuts for current mode

### **Visual Feedback**
```
Folder: INBOX | Emails: 150 | Last sync: 14:30:25 | Press 'r' to refresh, 'f' for folders
```

During sync:
```
Folder: INBOX | Emails: 150 | Syncing... | Press 'r' to refresh, 'f' for folders
```

## ⌨️ **Keyboard Shortcuts**

### **In Normal Mode (Email List)**
- **'r'**: Refresh current folder
- **'f'**: Switch to folder list
- **'c'**: Compose new email
- **'?'**: Show help

### **In Folder List Mode**
- **↑/↓**: Navigate folders (auto-syncs each folder)
- **Enter**: Select folder and return to email list
- **Esc**: Cancel and return to email list

## 🚀 **User Experience**

### **Seamless Navigation**
1. **Press 'f'** to open folder list
2. **Use ↑/↓** to browse folders
   - Each folder automatically syncs as you navigate
   - Status bar shows sync progress
3. **Press Enter** to select a folder
4. **View updated emails** immediately

### **Always Up-to-Date**
- **No manual refresh needed**: Folders sync automatically when accessed
- **Real-time status**: Always know when data was last updated
- **Offline fallback**: Shows cached emails if server unavailable

### **Efficient Bandwidth Usage**
- **Incremental sync**: Only downloads new emails
- **Smart caching**: Reuses cached data when possible
- **Background processing**: Sync doesn't block UI interaction

## 🔧 **Technical Implementation**

### **Auto-Sync Logic**
```rust
// Folder navigation with auto-sync
KeyCode::Up => {
    if !self.folders.is_empty() && self.selected_folder_idx > 0 {
        self.selected_folder_idx -= 1;
        // Auto-sync when switching folders
        if let Err(e) = self.load_emails() {
            self.show_error(&format!("Failed to load emails: {}", e));
        }
    }
}
```

### **Sync Status Tracking**
```rust
pub struct App {
    pub last_sync: Option<DateTime<Local>>,
    pub is_syncing: bool,
    // ... other fields
}
```

### **Enhanced Status Display**
```rust
// Show sync status in status bar
if app.is_syncing {
    text.push_str("Syncing... | ");
} else if let Some(last_sync) = app.last_sync {
    text.push_str(&format!("Last sync: {} | ", last_sync.format("%H:%M:%S")));
}
```

## 📈 **Benefits**

### **Before Auto-Sync**
- ❌ Manual refresh required to see new emails
- ❌ No indication of data freshness
- ❌ Folder switching showed stale data
- ❌ No sync status feedback

### **After Auto-Sync**
- ✅ Automatic sync when navigating folders
- ✅ Real-time sync status display
- ✅ Always shows fresh data
- ✅ Clear feedback on sync progress
- ✅ Manual refresh option available
- ✅ Efficient incremental updates

## 🎯 **Usage Scenarios**

### **Daily Email Management**
1. **Start application**: INBOX syncs automatically
2. **Check other folders**: Navigate with 'f', each folder syncs as you browse
3. **Stay updated**: Press 'r' anytime to refresh current folder
4. **Visual confirmation**: Status bar shows last sync time

### **Heavy Email Users**
- **Multiple folders**: Each folder syncs independently
- **Large mailboxes**: Incremental sync keeps it fast
- **Frequent updates**: Auto-sync ensures you never miss new emails
- **Offline capability**: Cached emails available when server down

### **Occasional Users**
- **Simple navigation**: Just browse folders, sync happens automatically
- **No learning curve**: Works intuitively without manual refresh
- **Status awareness**: Always know if data is current

The auto-sync system transforms the email client from a manual-refresh tool into an intelligent, always-current email management system that keeps users informed and up-to-date effortlessly.
