import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui

// Reads the scene manager's state file. No IPC dependency, so the widget
// stays quiet when the daemon is stopped.
BarWidget {
  id: root
  moduleName: "io.github.kill74.scenes"

  readonly property string statePath: (Quickshell.env("XDG_STATE_HOME")
    || (Quickshell.env("HOME") || "") + "/.local/state")
    + "/omarchy-scenes/state.json"
  property string scene: ""
  property int pausedUntil: 0
  property int nowSec: Math.floor(Date.now() / 1000)
  property bool hovered: area.containsMouse
  property string lastText: ""

  readonly property bool paused: root.pausedUntil > root.nowSec
  readonly property color accent: Commons.Color.accent
  readonly property color ink: root.bar ? root.bar.foreground : "#ffffff"
  readonly property color statusColor: root.paused ? "#f5b544" : root.accent
  readonly property int remaining: Math.max(0, root.pausedUntil - root.nowSec)
  readonly property string remainingLabel: root.remaining >= 3600
    ? Math.ceil(root.remaining / 3600) + "h"
    : (root.remaining >= 60 ? Math.ceil(root.remaining / 60) + "m" : root.remaining + "s")
  readonly property string label: root.paused ? "paused " + root.remainingLabel : root.scene

  function clear() {
    root.scene = ""
    root.pausedUntil = 0
  }

  function parse(text) {
    try {
      var data = JSON.parse(String(text || ""))
      root.scene = String(data.active_scene || "idle")
      root.pausedUntil = Number(data.paused_until || 0)
      root.nowSec = Math.floor(Date.now() / 1000)
    } catch (e) {
      root.clear()
    }
  }

  FileView {
    id: stateFile
    path: root.statePath
    watchChanges: true
    printErrors: false
    onLoaded: {
      root.lastText = text()
      root.parse(root.lastText)
    }
    onLoadFailed: root.clear()
  }

  // The daemon writes the file with an atomic rename, so watching the file
  // alone can miss updates (and cannot see it appear at all). Watch the
  // directory as the bar does, and poll as a fallback if the watch goes quiet.
  FileView {
    path: (Quickshell.env("XDG_STATE_HOME")
      || (Quickshell.env("HOME") || "") + "/.local/state")
      + "/omarchy-scenes"
    watchChanges: true
    printErrors: false
    onFileChanged: stateFile.reload()
  }

  Timer {
    interval: 1000
    running: true
    repeat: true
    onTriggered: stateFile.reload()
  }

  // Tick the pause countdown, and re-evaluate the deadline on a longer beat
  // even when nothing else changes.
  Timer {
    interval: 1000
    running: root.paused
    repeat: true
    onTriggered: root.nowSec = Math.floor(Date.now() / 1000)
  }

  Timer {
    interval: 30000
    running: true
    repeat: true
    onTriggered: if (root.lastText) root.parse(root.lastText)
  }

  // ---- Popup. The bar identifies panels by the widget mounted in its slot,
  //      so all the open/close identity lives here and is forwarded to Panel.
  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false
  readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false

  function open() { if (panelLoader.item) panelLoader.item.open() }
  function close() { if (panelLoader.item) panelLoader.item.close() }
  function togglePanel() { if (panelLoader.item) panelLoader.item.toggle() }
  function closeForPopoutSwitch() {
    if (panelLoader.item) panelLoader.item.closeForPopoutSwitch()
  }

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    if ("bar" in target) target.bar = root.bar
    if ("settings" in target) target.settings = root.settings
    if ("anchorItem" in target) target.anchorItem = capsule
    if ("hostWidget" in target) target.hostWidget = root
  }

  onBarChanged: injectPanel()
  onSettingsChanged: injectPanel()

  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    onLoaded: {
      root.injectPanel()
      Qt.callLater(root.injectPanel)
    }
  }

  ShellIpc {
    target: "io.github.kill74.scenes"
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.togglePanel() }
  }

  visible: root.scene !== ""
  implicitWidth: visible ? capsule.implicitWidth + 12 : 0
  implicitHeight: root.barSize

  Rectangle {
    id: capsule
    anchors.centerIn: parent
    implicitWidth: content.implicitWidth + 20
    width: implicitWidth
    height: Math.min(parent.height - 6, 22)
    radius: height / 2
    color: Qt.alpha(root.statusColor, root.hovered ? 0.26 : 0.13)
    border.width: 1
    border.color: Qt.alpha(root.statusColor, root.hovered ? 0.70 : 0.32)

    Row {
      id: content
      anchors.centerIn: parent
      spacing: 7

      // A two-ring beacon: halo plus core, tinted by the current state.
      Item {
        width: 10
        height: 10
        anchors.verticalCenter: parent.verticalCenter

        Rectangle {
          anchors.centerIn: parent
          width: 10
          height: 10
          radius: 5
          color: "transparent"
          border.width: 1
          border.color: root.statusColor
          opacity: 0.55
        }
        Rectangle {
          anchors.centerIn: parent
          width: 4
          height: 4
          radius: 2
          color: root.statusColor
        }
      }

      Text {
        anchors.verticalCenter: parent.verticalCenter
        text: root.label.toUpperCase()
        textFormat: Text.PlainText
        color: root.ink
        font.family: root.bar ? root.bar.fontFamily : "Adwaita Sans"
        font.pixelSize: 10
        font.bold: true
        font.letterSpacing: 1.2
        opacity: root.hovered ? 1.0 : 0.9
      }
    }
  }

  MouseArea {
    id: area
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onClicked: root.togglePanel()
    onEntered: if (root.bar) root.bar.showTooltip(root, "Scenes: " + root.label)
    onExited: if (root.bar) root.bar.hideTooltip(root)
  }
}
