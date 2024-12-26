#include <QFileDialog>
#include <QString>
#include <QStringList>
#include "../inc/util.h"
#include "front.h"

class Picker {
public:
    Picker() : dialog(nullptr), url() {}
    ~Picker() = default;

    QFileDialog* dialog;
    QString url;
    QByteArray urlUtf8;
};

static QStringList
parseAllowedTypes(uint8_t const* mask) {
    QStringList types;
    if (!mask) {
        return types;
    }

    QString extensions = QString::fromUtf8(reinterpret_cast<const char*>(mask));
    types = extensions.split('|', Qt::SkipEmptyParts);

    for (int i = 0; i < types.size(); ++i) {
        types[i] = "*." + types[i];
    }

    return types;
}

extern "C" void*
back_file_open_picker_init(uint8_t const* allowed_mask) {
    Picker* picker = new Picker();
    picker->dialog = new QFileDialog();

    QStringList filters = parseAllowedTypes(allowed_mask);
    if (!filters.isEmpty()) {
        picker->dialog->setNameFilters(filters);
    }

    picker->dialog->setFileMode(QFileDialog::ExistingFile);
    return picker;
}

extern "C" uint8_t const*
back_file_open_picker_run(void* op) {
    Picker* picker = static_cast<Picker*>(op);
    if (picker->dialog->exec() == QDialog::Accepted) {
        QStringList files = picker->dialog->selectedFiles();
        if (!files.isEmpty()) {
            picker->url = files.first();
            picker->urlUtf8 = picker->url.toUtf8();
            return reinterpret_cast<const uint8_t*>(picker->urlUtf8.constData());
        }
    }
    return nullptr;
}

extern "C" void
back_file_open_picker_free(void* op) {
    Picker* picker = static_cast<Picker*>(op);
    delete picker->dialog;
    delete picker;
}

extern "C" void*
back_file_save_picker_init(uint8_t const* allowed_mask) {
    Picker* picker = new Picker();
    picker->dialog = new QFileDialog();

    QStringList filters = parseAllowedTypes(allowed_mask);
    if (!filters.isEmpty()) {
        picker->dialog->setNameFilters(filters);
    }

    picker->dialog->setFileMode(QFileDialog::AnyFile);
    picker->dialog->setAcceptMode(QFileDialog::AcceptSave);
    return picker;
}

extern "C" uint8_t const*
back_file_save_picker_run(void* op) {
    return back_file_open_picker_run(op);
}

extern "C" void
back_file_save_picker_free(void* op) {
    back_file_open_picker_free(op);
}