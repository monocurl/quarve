#import <Cocoa/Cocoa.h>
#import "util.h"
#import "../inc/front.h"


@interface Dropdown : NSPopUpButton
@property fat_pointer binding;
@property BOOL in_transaction;
- (void)addOption:(NSString *)option;
- (uint8_t)selectOption:(NSString *)selection;
@end

@implementation Dropdown
- (void)addOption:(NSString *)option {
    [self addItemWithTitle:option];
}

- (uint8_t)selectOption:(NSString *)selection {
    NSInteger index = [self indexOfItemWithTitle:selection];
    if (index != -1) {
        [self selectItemAtIndex:index];
        return 0;
    }
    return 1;
}

- (void)itemChanged:(id)sender {
    if (!self.in_transaction) {
        NSMenuItem* item = [self selectedItem];
        if (item == NULL) {
            front_set_opt_string_binding(self.binding, NULL);
        }
        else {
            front_set_opt_string_binding(self.binding, (uint8_t*) [item.title UTF8String]);
        }
    }
}

- (void)dealloc {
    [super dealloc];

    front_free_opt_string_binding(self.binding);
}
@end

void*
back_view_dropdown_init(fat_pointer binding) {
    Dropdown* dd = [[Dropdown alloc] init];
    dd.binding = binding;
    dd.in_transaction = NO;

    [dd setTarget:dd];
    [dd setAction:@selector(itemChanged:)];

    return (void *)dd;
}

void
back_view_dropdown_add(void *_view, unsigned char const* option) {
    Dropdown *dd = (Dropdown*) _view;
    NSString *optionString = [NSString stringWithUTF8String:(const char *)option];
    [dd addOption:optionString];
}

uint8_t
back_view_dropdown_select(void *_view, unsigned char const* selection) {
    Dropdown *dropdown = (Dropdown *) _view;

    if (selection) {
        dropdown.in_transaction = YES;

        NSString *selectionString = [NSString stringWithUTF8String:(const char *)selection];
        uint8_t ret = [dropdown selectOption:selectionString];
        dropdown.in_transaction = NO;

        return ret;
    }
    else {
        [dropdown selectItem:NULL];
        return 0;
    }
}

void
back_view_dropdown_clear(void* _view)
{
    Dropdown* dd = (Dropdown*) _view;
    [dd removeAllItems];
}

size
back_view_dropdown_size(void *_view) {
    Dropdown *dropdown = (Dropdown *)_view;
    NSSize ret = [dropdown intrinsicContentSize];
    return (size) { ret.width, ret.height };
}
