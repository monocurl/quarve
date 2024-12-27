#include <QMenu>
#include <QMenuBar>
#include <QAction>
#include <QString>

#include "../inc/util.h"
#include "debug.h"
#include "front.h"

class Button : public QAction {
public:
    Button(QObject* parent = nullptr) : QAction(parent), callback{NULL, NULL} {}
    ~Button() {
        if (callback.p0 != NULL) {
            front_free_fn_mut(callback);
        }
    }

    fat_pointer callback;
};

extern "C" void*
back_menu_bar_init() {
    QMenuBar *mb = new QMenuBar();
    return mb;
}

extern "C" void
back_menu_bar_add(void* menu_bar, void* menu_item, uint8_t const* title)
{
    QMenuBar *mb = (QMenuBar*) menu_bar;
    QMenu *item = (QMenu*) menu_item;
    mb->addMenu(item);
}

extern "C" void
back_menu_bar_free(void* menu_bar) {
    QMenuBar *mb = (QMenuBar*) menu_bar;
    delete mb;
}

extern "C" void*
back_menu_init(uint8_t const* title) {
    QMenu* menu = new QMenu(QString::fromUtf8(reinterpret_cast<const char*>(title)));
    return menu;
}

extern "C" void
back_menu_add(void* _menu, void* _item) {
    QMenu* menu = static_cast<QMenu*>(_menu);
    QAction* item = static_cast<QAction*>(_item);
    menu->addAction(item);
}

extern "C" void
back_menu_free(void* _menu) {
    QMenu* menu = static_cast<QMenu*>(_menu);
    delete menu;
}

extern "C" void*
back_menu_separator_init() {
    QAction* separator = new QAction();
    separator->setSeparator(true);
    return separator;
}

extern "C" void
back_menu_separator_free(void* _separator) {
    QAction* separator = static_cast<QAction*>(_separator);
    delete separator;
}

extern "C" void*
back_menu_button_init(uint8_t const* title, uint8_t const* keyEquivalent, uint8_t modifiers) {
    Button* button = new Button();
    button->setText(QString::fromUtf8(reinterpret_cast<const char*>(title)));

    // Convert key equivalent to Qt shortcut
    QString shortcut = QString::fromUtf8(reinterpret_cast<const char*>(keyEquivalent));
    QKeySequence sequence;

    if (!shortcut.isEmpty()) {
        Qt::KeyboardModifiers qtModifiers = Qt::NoModifier;

        if (modifiers & EVENT_MODIFIER_CONTROL) {
            qtModifiers |= Qt::ControlModifier;
        }
        if (modifiers & EVENT_MODIFIER_META) {
            qtModifiers |= Qt::MetaModifier;
        }
        if (modifiers & EVENT_MODIFIER_SHIFT) {
            qtModifiers |= Qt::ShiftModifier;
        }
        if (modifiers & EVENT_MODIFIER_ALT_OPTION) {
            qtModifiers |= Qt::AltModifier;
        }

        sequence = QKeySequence(qtModifiers | shortcut[0].unicode());
        button->setShortcut(sequence);
    }

    QObject::connect(button, &QAction::triggered, [button]{
        if (button->callback.p0 != NULL) {
            front_execute_fn_mut(button->callback);
        }
    });
    return button;
}

extern "C" void
back_menu_button_set_title(void* _button, uint8_t const* title) {
    Button* button = static_cast<Button*>(_button);
    button->setText(QString::fromUtf8(reinterpret_cast<const char*>(title)));
}

extern "C" void
back_menu_button_set_action(void* _button, fat_pointer action) {
    Button* button = static_cast<Button*>(_button);
    if (button->callback.p0 != NULL) {
        front_free_fn_mut(button->callback);
    }
    button->callback = action;
}

extern "C" void
back_menu_button_set_enabled(void* _button, uint8_t enabled) {
    Button* button = static_cast<Button*>(_button);
    button->setEnabled(enabled != 0);
}

extern "C" void
back_menu_button_set_submenu(void* _button, void* _menu) {
    Button* button = static_cast<Button*>(_button);
    QMenu* menu = static_cast<QMenu*>(_menu);
    button->setMenu(menu);
}

extern "C" void
back_menu_button_free(void* _button) {
    Button* button = static_cast<Button*>(_button);
    delete button;
}