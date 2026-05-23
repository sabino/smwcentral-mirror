#include <QApplication>
#include <QComboBox>
#include <QCoreApplication>
#include <QDir>
#include <QFileDialog>
#include <QFileInfo>
#include <QFormLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLabel>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QProcess>
#include <QPushButton>
#include <QTableWidget>
#include <QTabWidget>
#include <QVBoxLayout>
#include <QWidget>

#include <cstdlib>

class MainWindow final : public QWidget {
public:
  MainWindow() {
    setWindowTitle("smwapt");
    resize(1120, 760);

    auto *layout = new QVBoxLayout(this);
    auto *top = new QHBoxLayout();
    rootEdit_ = new QLineEdit(QDir::currentPath());
    binaryEdit_ = new QLineEdit(defaultBinary());
    auto *browseRoot = new QPushButton("Browse");
    connect(browseRoot, &QPushButton::clicked, this, [this]() {
      const auto dir = QFileDialog::getExistingDirectory(this, "Project root", rootEdit_->text());
      if (!dir.isEmpty()) rootEdit_->setText(dir);
    });
    top->addWidget(new QLabel("Project"));
    top->addWidget(rootEdit_, 2);
    top->addWidget(browseRoot);
    top->addWidget(new QLabel("smwapt"));
    top->addWidget(binaryEdit_, 2);
    layout->addLayout(top);

    auto *tabs = new QTabWidget();
    tabs->addTab(repoTab(), "Repository");
    tabs->addTab(projectTab(), "Project");
    tabs->addTab(outputTab(), "Output");
    layout->addWidget(tabs);
  }

private:
  QWidget *repoTab() {
    auto *page = new QWidget();
    auto *layout = new QVBoxLayout(page);

    auto *sources = new QGroupBox("Sources");
    auto *sourceLayout = new QHBoxLayout(sources);
    sourceUrl_ = new QLineEdit("http://127.0.0.1:4789");
    auto *addSource = new QPushButton("Add");
    auto *listSources = new QPushButton("List");
    auto *update = new QPushButton("Update");
    sourceLayout->addWidget(sourceUrl_, 1);
    sourceLayout->addWidget(addSource);
    sourceLayout->addWidget(listSources);
    sourceLayout->addWidget(update);
    connect(addSource, &QPushButton::clicked, this, [this]() {
      run({"source", "add", sourceUrl_->text(), "stable", "main"});
    });
    connect(listSources, &QPushButton::clicked, this, [this]() { run({"source", "list"}); });
    connect(update, &QPushButton::clicked, this, [this]() { run({"update"}); });
    layout->addWidget(sources);

    auto *searchLayout = new QHBoxLayout();
    searchEdit_ = new QLineEdit();
    searchEdit_->setPlaceholderText("asar, retry, uberasm, sprite...");
    auto *search = new QPushButton("Search");
    auto *show = new QPushButton("Show");
    searchLayout->addWidget(searchEdit_, 1);
    searchLayout->addWidget(search);
    searchLayout->addWidget(show);
    layout->addLayout(searchLayout);
    connect(search, &QPushButton::clicked, this, [this]() { run({"search", searchEdit_->text()}); });
    connect(show, &QPushButton::clicked, this, [this]() { run({"show", searchEdit_->text()}); });

    auto *installBox = new QGroupBox("Install");
    auto *form = new QFormLayout(installBox);
    packageEdit_ = new QLineEdit();
    entryEdit_ = new QLineEdit();
    targetEdit_ = new QLineEdit();
    map16Edit_ = new QLineEdit();
    spriteSlotEdit_ = new QLineEdit();
    songSlotEdit_ = new QLineEdit();
    auto *dryRun = new QPushButton("Dry Run");
    auto *install = new QPushButton("Install");
    auto *buttons = new QHBoxLayout();
    buttons->addWidget(dryRun);
    buttons->addWidget(install);
    form->addRow("Package", packageEdit_);
    form->addRow("Entry", entryEdit_);
    form->addRow("UberASM target", targetEdit_);
    form->addRow("GPS Map16", map16Edit_);
    form->addRow("PIXI slot", spriteSlotEdit_);
    form->addRow("AMK slot", songSlotEdit_);
    form->addRow(buttons);
    connect(dryRun, &QPushButton::clicked, this, [this]() { installPackage(true); });
    connect(install, &QPushButton::clicked, this, [this]() { installPackage(false); });
    layout->addWidget(installBox);
    layout->addStretch();
    return page;
  }

  QWidget *projectTab() {
    auto *page = new QWidget();
    auto *layout = new QVBoxLayout(page);
    auto *initBox = new QGroupBox("ROM");
    auto *form = new QFormLayout(initBox);
    romEdit_ = new QLineEdit("/home/sabino/Downloads/Super Mario World (USA) (2).sfc");
    copyRomEdit_ = new QLineEdit("./hack.sfc");
    auto *browseRom = new QPushButton("Browse ROM");
    auto *verify = new QPushButton("Verify");
    auto *init = new QPushButton("Init");
    auto *row = new QHBoxLayout();
    row->addWidget(browseRom);
    row->addWidget(verify);
    row->addWidget(init);
    form->addRow("Source ROM", romEdit_);
    form->addRow("Project copy", copyRomEdit_);
    form->addRow(row);
    connect(browseRom, &QPushButton::clicked, this, [this]() {
      const auto file = QFileDialog::getOpenFileName(this, "SMW ROM", QDir::homePath(), "SNES ROM (*.sfc *.smc)");
      if (!file.isEmpty()) romEdit_->setText(file);
    });
    connect(verify, &QPushButton::clicked, this, [this]() { run({"rom", "verify", "--rom", romEdit_->text()}); });
    connect(init, &QPushButton::clicked, this, [this]() {
      run({"init", "--rom", romEdit_->text(), "--copy-rom", copyRomEdit_->text()});
    });
    layout->addWidget(initBox);

    auto *ops = new QHBoxLayout();
    auto *doctor = new QPushButton("Doctor");
    auto *history = new QPushButton("History");
    auto *list = new QPushButton("List Installed");
    ops->addWidget(doctor);
    ops->addWidget(history);
    ops->addWidget(list);
    ops->addStretch();
    connect(doctor, &QPushButton::clicked, this, [this]() { run({"doctor"}); });
    connect(history, &QPushButton::clicked, this, [this]() { run({"history"}); });
    connect(list, &QPushButton::clicked, this, [this]() { run({"list"}); });
    layout->addLayout(ops);
    layout->addStretch();
    return page;
  }

  QWidget *outputTab() {
    auto *page = new QWidget();
    auto *layout = new QVBoxLayout(page);
    output_ = new QPlainTextEdit();
    output_->setReadOnly(true);
    auto *clear = new QPushButton("Clear");
    connect(clear, &QPushButton::clicked, output_, &QPlainTextEdit::clear);
    layout->addWidget(output_, 1);
    layout->addWidget(clear);
    return page;
  }

  void installPackage(bool dryRun) {
    QStringList args{"install", packageEdit_->text()};
    addOpt(args, "--entry", entryEdit_->text());
    addOpt(args, "--target", targetEdit_->text());
    addOpt(args, "--map16", map16Edit_->text());
    addOpt(args, "--sprite-slot", spriteSlotEdit_->text());
    addOpt(args, "--song-slot", songSlotEdit_->text());
    if (dryRun) args << "--dry-run";
    run(args);
  }

  void run(QStringList args) {
    auto *process = new QProcess(this);
    QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    if (!env.value("WAYLAND_DISPLAY").isEmpty()) {
      if (env.value("QT_QPA_PLATFORM").isEmpty()) env.insert("QT_QPA_PLATFORM", "wayland;xcb");
      if (env.value("SDL_VIDEODRIVER").isEmpty()) env.insert("SDL_VIDEODRIVER", "wayland");
      if (env.value("GDK_BACKEND").isEmpty()) env.insert("GDK_BACKEND", "wayland,x11");
    }
    if (env.value("NO_AT_BRIDGE").isEmpty()) env.insert("NO_AT_BRIDGE", "1");
    process->setProcessEnvironment(env);
    process->setWorkingDirectory(rootEdit_->text());
    args.prepend(rootEdit_->text());
    args.prepend("--root");
    output_->appendPlainText("$ " + binaryEdit_->text() + " " + args.join(' '));
    connect(process, &QProcess::readyReadStandardOutput, this, [this, process]() {
      output_->appendPlainText(QString::fromLocal8Bit(process->readAllStandardOutput()));
    });
    connect(process, &QProcess::readyReadStandardError, this, [this, process]() {
      output_->appendPlainText(QString::fromLocal8Bit(process->readAllStandardError()));
    });
    connect(process, &QProcess::finished, this, [this, process](int code, QProcess::ExitStatus status) {
      output_->appendPlainText(QString("exit=%1 status=%2").arg(code).arg(status == QProcess::NormalExit ? "normal" : "crash"));
      process->deleteLater();
    });
    process->start(binaryEdit_->text(), args);
  }

  static void addOpt(QStringList &args, const QString &flag, const QString &value) {
    if (!value.trimmed().isEmpty()) args << flag << value.trimmed();
  }

  static QString defaultBinary() {
    const auto appDir = QCoreApplication::applicationDirPath();
    const QString local = QDir(appDir).absoluteFilePath("smwapt");
    if (QFileInfo::exists(local)) return local;
    const QString dev = QDir(appDir).absoluteFilePath("../../target/debug/smwapt");
    if (QFileInfo::exists(dev)) return QFileInfo(dev).canonicalFilePath();
    return "smwapt";
  }

  QLineEdit *rootEdit_{};
  QLineEdit *binaryEdit_{};
  QLineEdit *sourceUrl_{};
  QLineEdit *searchEdit_{};
  QLineEdit *packageEdit_{};
  QLineEdit *entryEdit_{};
  QLineEdit *targetEdit_{};
  QLineEdit *map16Edit_{};
  QLineEdit *spriteSlotEdit_{};
  QLineEdit *songSlotEdit_{};
  QLineEdit *romEdit_{};
  QLineEdit *copyRomEdit_{};
  QPlainTextEdit *output_{};
};

int main(int argc, char **argv) {
  if (qEnvironmentVariableIsSet("WAYLAND_DISPLAY") && !qEnvironmentVariableIsSet("QT_QPA_PLATFORM")) {
    qputenv("QT_QPA_PLATFORM", "wayland;xcb");
  }
  if (qEnvironmentVariableIsSet("WAYLAND_DISPLAY") && !qEnvironmentVariableIsSet("SDL_VIDEODRIVER")) {
    qputenv("SDL_VIDEODRIVER", "wayland");
  }
  if (!qEnvironmentVariableIsSet("NO_AT_BRIDGE")) qputenv("NO_AT_BRIDGE", "1");

  QApplication app(argc, argv);
  MainWindow window;
  window.show();
  return app.exec();
}
