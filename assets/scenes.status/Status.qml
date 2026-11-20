import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
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
  property bool paused: false

  readonly property color ink: root.bar ? root.bar.foreground : "white"

  function clear() {
    root.scene = ""
    root.paused = false
  }

  function parse(text) {
    try {
      var data = JSON.parse(String(text || ""))
      root.scene = String(data.active_scene || "idle")
      root.paused = Number(data.paused_until || 0) > Math.floor(Date.now() / 1000)
    } catch (e) {
      root.clear()
    }
  }

  FileView {
    path: root.statePath
    watchChanges: true
    printErrors: false
    onLoaded: root.parse(text())
    onLoadFailed: root.clear()
  }

  visible: root.scene !== ""
  implicitWidth: visible ? row.implicitWidth + 14 : 0
  implicitHeight: root.barSize

  Row {
    id: row
    anchors.centerIn: parent
    spacing: 6

    Text {
      anchors.verticalCenter: parent.verticalCenter
      text: root.paused ? "⏸" : "◉"
      textFormat: Text.PlainText
      color: root.ink
      font.family: "Noto Sans"
      font.pixelSize: 10
      opacity: 0.75
    }

    Text {
      anchors.verticalCenter: parent.verticalCenter
      text: root.paused ? "paused" : root.scene
      textFormat: Text.PlainText
      color: root.ink
      font.family: "Adwaita Sans"
      font.pixelSize: 11
      opacity: 0.85
    }
  }

  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onClicked: if (root.bar) root.bar.run("omarchy menu summon scenes")
    onEntered: if (root.bar) root.bar.showTooltip(root, "Scenes: " + (root.paused ? "paused" : root.scene))
    onExited: if (root.bar) root.bar.hideTooltip(root)
  }
}
