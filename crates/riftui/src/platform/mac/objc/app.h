#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <UserNotifications/UserNotifications.h>

// Our NSApplication subclass.
@interface RiftApplication : NSApplication
@end

// RiftDelegate is the delegate of the NSApp and also all menus.
@interface RiftDelegate
    : NSObject <NSApplicationDelegate, NSMenuDelegate, UNUserNotificationCenterDelegate>

@property(strong) NSMenu *dockMenu;

@end

// Functions implemented in Rust.
void rift_app_will_finish_launching(id app);
void rift_app_did_become_active(id app);
void rift_app_did_resign_active(id app);
void rift_app_will_terminate(id app);
void rift_app_open_files(id app, id filenames);
void rift_app_send_global_keybinding(id app, NSUInteger modifiers, NSUInteger key_code);
void rift_app_new_window(id app);
void rift_app_window_did_resize(id app);
void rift_app_window_did_move(id app);
void rift_app_window_will_close(id app, id window);
void rift_app_screen_did_change(id app);
void cpu_awakened(id app);
void cpu_will_sleep(id app);
void rift_app_active_window_changed(id app);
void rift_app_notification_clicked(id app, double date, id data);
void rift_app_open_urls(id app, id urls);
void rift_app_os_appearance_changed(id app);
BOOL rift_app_should_terminate_app(id app);
BOOL rift_app_should_close_window(id app, id window);
BOOL rift_app_are_key_bindings_disabled_for_window(id app, id window);
BOOL rift_app_has_binding_for_keystroke(id app, id event);
BOOL rift_app_has_custom_action_for_keystroke(id app, id event);
void rift_app_disable_warning_modal(id app);
void rift_app_internet_reachability_changed(id app, BOOL can_reach);
void rift_app_process_modal_response(id app, NSUInteger modal_id, NSModalResponse response,
                                     BOOL disable_modal);
