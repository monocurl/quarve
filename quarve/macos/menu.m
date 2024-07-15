#import <Cocoa/Cocoa.h>
#import "util.h"
#import "front.h"

@interface Button : NSMenuItem
@property fat_pointer callback;
@end

@implementation Button

-(void)perform:(id) sender {
    if (self.callback.p0 != NULL) {
        front_execute_fn_mut(self.callback);
    }
}

-(void)dealloc {
    [super dealloc];
    if (self.callback.p0 != NULL) {
        front_free_fn_mut(self.callback);
    }
}

@end

void*
back_menu_init(uint8_t const* title)
{
    return [[NSMenu alloc] initWithTitle: [NSString stringWithUTF8String: (char const*) title]];
}

void
back_menu_add(void *_menu, void* _item) {
    NSMenu* menu = _menu;
    Button* item = _item;
    [menu addItem:item];
}

void
back_menu_free(void *_menu) {
    NSMenu* menu = _menu;
    [menu release];
}

// button
void*
back_menu_button_init(uint8_t const* title, uint8_t const* keyEquivalent, uint8_t modifiers)
{
    Button* ret = [[Button alloc]
        initWithTitle:[NSString stringWithUTF8String:(char const*) title]
        action: @selector(perform:)
        keyEquivalent:[NSString stringWithUTF8String:(char const*) keyEquivalent]
    ];
    ret.callback = (fat_pointer) {NULL, NULL};
    ret.target = ret;
    ret.keyEquivalentModifierMask = 0;
    if (modifiers & EVENT_MODIFIER_COMMAND) {
        ret.keyEquivalentModifierMask |= NSEventModifierFlagCommand;
    }
    if (modifiers & EVENT_MODIFIER_CONTROL) {
        ret.keyEquivalentModifierMask |= NSEventModifierFlagControl;
    }
    if (modifiers & EVENT_MODIFIER_SHIFT) {
        ret.keyEquivalentModifierMask |= NSEventModifierFlagShift;
    }
    if (modifiers & EVENT_MODIFIER_FN) {
        ret.keyEquivalentModifierMask |= NSEventModifierFlagFunction;
    }
    if (modifiers & EVENT_MODIFIER_ALT_OPTION) {
        ret.keyEquivalentModifierMask |= NSEventModifierFlagOption;
    }
    return ret;
}

void
back_menu_button_set_title(void* _button, uint8_t const* title)
{
    Button* button = _button;
    button.title = [NSString stringWithUTF8String:(char const*)title];
}

void
back_menu_button_set_action(void *_button, fat_pointer action)
{
    Button* button = _button;
    button.callback = action;
}

void
back_menu_button_set_enabled(void *_button, uint8_t enabled)
{
    Button* button = _button;
    button.enabled = enabled != 0;
}

void
back_menu_button_set_submenu(void *_button, void* _menu)
{
    Button* button = _button;
    NSMenu* menu = _menu;
    button.submenu = menu;
}

void
back_menu_button_free(void *_button)
{
    Button* button = _button;
    [button release];
}