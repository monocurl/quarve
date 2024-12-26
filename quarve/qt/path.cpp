#include <cstdlib>
#include <cassert>

#include <QStandardPaths>
#include <QDir>

extern "C" uint8_t const*
back_app_storage_directory(uint8_t const* app_name) {
    static thread_local QString threadLocalStorageDirectory;
    static thread_local QByteArray threadLocalStorageString;

    if (threadLocalStorageDirectory.isEmpty()) {
        QString basePath = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);

        if (!basePath.isEmpty()) {
            QString appNameStr = QString::fromUtf8(reinterpret_cast<const char*>(app_name));

            QString appPath = basePath + QDir::separator() + appNameStr;

            QDir dir;
            if (!dir.exists(appPath)) {
                if (!dir.mkpath(appPath)) {
                    assert(0);
                }
            }

            threadLocalStorageDirectory = appPath;
            threadLocalStorageString = threadLocalStorageDirectory.toUtf8();
        }
        else {
            assert(0);
        }
    }

    return reinterpret_cast<const uint8_t*>(threadLocalStorageString.constData());
}