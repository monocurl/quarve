#pragma once

@interface CursorView : NSView
- (instancetype)initWithCursor:(NSCursor *)cursor;
@property (strong) NSCursor *cursor;
@end

