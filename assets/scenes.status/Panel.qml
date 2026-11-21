import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui

// Scenes popup: a compact read-out of the current context with one-click
// controls. The bar-widget owns the IPC identity; this panel only paints and
// calls the CLI, so it stays useful even if the shell is restarted.
Panel {
  id: root
  moduleName: "io.github.kill74.scenes"
  ipcTarget: "io.github.kill74.scenes"
  manageIpc: false

  property var anchorItem: null
  property var hostWidget: null
  readonly property var barIdentity: hostWidget || root

  readonly property string scenesBin: (Quickshell.env("HOME") || "") + "/.local/bin/scenes"
  readonly property color foreground: bar ? bar.foreground : Commons.Color.foreground
  readonly property color accent: Commons.Color.accent
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property var snapshot: root.data && root.data.state ? root.data : ({})

  property var data: ({})
  property string error: ""
  property int nowSec: Math.floor(Date.now() / 1000)
  property bool settling: false

  readonly property bool paused: Number((root.snapshot.paused_until || 0)) > root.nowSec
  readonly property int remaining: Math.max(0, Number(root.snapshot.paused_until || 0) - root.nowSec)
  readonly property string remainingLabel: root.remaining >= 3600
    ? Math.ceil(root.remaining / 3600) + "h"
    : (root.remaining >= 60 ? Math.ceil(root.remaining / 60) + "m" : root.remaining + "s")
  readonly property string activeName: root.snapshot.active_scene || "idle"

  function refresh(quickOnly) {
    probeQuick.running = true
    if (quickOnly !== true && !probe.running) probe.running = true
  }

  // Reflect a command before the daemon has written the file, then poll fast
  // until the real state arrives. Without this the panel only noticed a change
  // on its next ten-second refresh.
  function setLocal(changes) {
    var merged = {}
    var source = (root.data && root.data.state) ? root.data : {}
    for (var key in source) merged[key] = source[key]
    for (var change in changes) merged[change] = changes[change]
    merged.state = changes.state || merged.state || "idle"
    root.data = merged
  }

  function beginSettle(changes) {
    if (changes) root.setLocal(changes)
    root.settling = true
    settleStop.restart()
    root.refresh(true)
  }

  function run(command) {
    if (bar) bar.run(command)
  }

  function applyScene(name) {
    run(root.scenesBin + " apply " + Util.shellQuote(String(name)))
    root.beginSettle({ active_scene: String(name), state: "active" })
  }

  function leaveScene() {
    run(root.scenesBin + " off")
    root.beginSettle({ active_scene: null, state: "idle" })
  }

  function pause() {
    run(root.scenesBin + " pause 30m")
    root.beginSettle({ paused_until: Math.floor(Date.now() / 1000) + 1800, state: "paused" })
  }

  function resume() {
    run(root.scenesBin + " resume")
    root.beginSettle({
      paused_until: null,
      state: root.snapshot.active_scene ? "active" : "idle"
    })
  }

  function editConfig() {
    run("xdg-open ~/.config/omarchy/scenes/scenes.toml")
    root.close()
  }

  function signalLine(label, value) {
    return label + ": " + (value === null || value === undefined || value === "" ? "—" : value)
  }

  onOpenedChanged: if (root.opened) root.refresh()

  // Fast snapshot first (local files only), then the full probe for signals.
  Process {
    id: probeQuick
    command: [root.scenesBin, "panel", "--quick"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var parsed = JSON.parse(String(text || ""))
          if (parsed && parsed.state) {
            // Keep the signal rows from the last full probe; --quick omits them.
            var previousSignals = root.data ? root.data.signals : null
            if (previousSignals && (!parsed.signals || parsed.signals.monitors === null))
              parsed.signals = previousSignals
            root.data = parsed
            root.error = ""
          }
        } catch (e) {
          root.error = "Could not read scene data"
        }
      }
    }
    stderr: StdioCollector { waitForEnd: true }
  }

  Process {
    id: probe
    command: [root.scenesBin, "panel"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var parsed = JSON.parse(String(text || ""))
          if (parsed && parsed.state) {
            root.data = parsed
            root.error = ""
          }
        } catch (e) {
          root.error = "Could not read scene data"
        }
      }
    }
    stderr: StdioCollector { waitForEnd: true }
    onExited: function(code) {
      if (code !== 0 && !(root.data && root.data.state))
        root.error = "Could not read scene data"
    }
  }

  Timer {
    id: settleTimer
    interval: 150
    running: root.opened && root.settling
    repeat: true
    onTriggered: root.refresh(true)
  }

  Timer {
    id: settleStop
    interval: 2000
    repeat: false
    onTriggered: root.settling = false
  }

  Timer {
    interval: 1000
    running: root.opened && root.paused
    repeat: true
    onTriggered: root.nowSec = Math.floor(Date.now() / 1000)
  }

  Timer {
    interval: 10000
    running: root.opened
    repeat: true
    onTriggered: root.refresh()
  }

  PopupCard {
    id: popup
    anchorItem: root.anchorItem
    bar: root.bar
    owner: root.barIdentity
    open: root.opened
    centerOnBar: false
    contentWidth: popup.fittedContentWidth(Style.space(380))
    contentHeight: popup.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      implicitHeight: column.implicitHeight
      onCloseRequested: root.close()

      Column {
        id: column
        width: parent.width
        spacing: Style.space(8)

        // ---- header
        Item {
          width: parent.width
          height: 22

          Rectangle {
            id: headerDot
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: 8
            height: 8
            radius: 4
            color: root.paused ? "#f5b544" : root.accent
          }
          Text {
            anchors.left: headerDot.right
            anchors.leftMargin: Style.space(8)
            anchors.verticalCenter: parent.verticalCenter
            text: "SCENES"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: 11
            font.bold: true
            font.letterSpacing: 2
          }
          Button {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: "✕"
            onClicked: root.close()
          }
        }

        Rectangle {
          width: parent.width
          height: 1
          color: Qt.alpha(root.foreground, 0.12)
        }

        // ---- hero
        Column {
          width: parent.width
          spacing: Style.space(4)

          Text {
            text: root.activeName.toUpperCase()
            color: root.paused ? "#f5b544" : root.accent
            font.family: root.fontFamily
            font.pixelSize: 24
            font.bold: true
            font.letterSpacing: 1
          }
          Text {
            text: root.paused
              ? "Auto-switching paused · " + root.remainingLabel + " left"
              : (root.activeName === "idle" ? "No scene active" : "Scene active")
            color: Qt.alpha(root.foreground, 0.65)
            font.family: root.fontFamily
            font.pixelSize: 11
          }
        }

        // ---- signals
        Text {
          text: "LIVE SIGNALS"
          color: Qt.alpha(root.foreground, 0.5)
          font.family: root.fontFamily
          font.pixelSize: 9
          font.bold: true
          font.letterSpacing: 1.6
        }
        Grid {
          width: parent.width
          columns: 2
          rowSpacing: Style.space(3)
          columnSpacing: Style.space(10)

          Text { text: root.signalLine("Monitors", root.snapshot.signals ? root.snapshot.signals.monitors : null); color: Qt.alpha(root.foreground, 0.8); font.family: root.fontFamily; font.pixelSize: 10 }
          Text { text: root.signalLine("Wi-Fi", root.snapshot.signals ? root.snapshot.signals.ssid : null); color: Qt.alpha(root.foreground, 0.8); font.family: root.fontFamily; font.pixelSize: 10; elide: Text.ElideRight; width: 170 }
          Text { text: root.signalLine("Power", root.snapshot.signals ? root.snapshot.signals.power : null); color: Qt.alpha(root.foreground, 0.8); font.family: root.fontFamily; font.pixelSize: 10 }
          Text { text: root.signalLine("Battery", root.snapshot.signals && root.snapshot.signals.battery !== null ? root.snapshot.signals.battery + "%" : null); color: Qt.alpha(root.foreground, 0.8); font.family: root.fontFamily; font.pixelSize: 10 }
          Text { text: root.signalLine("Game", root.snapshot.signals ? String(root.snapshot.signals.game_active === true) : null); color: Qt.alpha(root.foreground, 0.8); font.family: root.fontFamily; font.pixelSize: 10 }
          Text { text: root.signalLine("Focus", root.snapshot.signals ? root.snapshot.signals.focused_cwd : null); color: Qt.alpha(root.foreground, 0.8); font.family: root.fontFamily; font.pixelSize: 10; elide: Text.ElideRight; width: 170 }
        }

        Rectangle {
          width: parent.width
          height: 1
          color: Qt.alpha(root.foreground, 0.12)
        }

        // ---- last decision
        Text {
          text: "LAST DECISION"
          color: Qt.alpha(root.foreground, 0.5)
          font.family: root.fontFamily
          font.pixelSize: 9
          font.bold: true
          font.letterSpacing: 1.6
        }
        Text {
          width: parent.width
          text: root.snapshot.last_decision
            ? root.snapshot.last_decision.scene + " · " + root.snapshot.last_decision.detail + "\n" + root.snapshot.last_decision.when
            : "No decisions logged yet"
          color: Qt.alpha(root.foreground, 0.8)
          font.family: root.fontFamily
          font.pixelSize: 10
          wrapMode: Text.Wrap
        }

        Rectangle {
          width: parent.width
          height: 1
          color: Qt.alpha(root.foreground, 0.12)
        }

        // ---- apply a scene
        Text {
          text: "APPLY A SCENE"
          color: Qt.alpha(root.foreground, 0.5)
          font.family: root.fontFamily
          font.pixelSize: 9
          font.bold: true
          font.letterSpacing: 1.6
        }
        Flow {
          width: parent.width
          spacing: Style.space(6)
          Repeater {
            model: root.snapshot.scenes || []
            Button {
              text: modelData
              bordered: true
              selected: modelData === root.activeName
              onClicked: root.applyScene(modelData)
            }
          }
        }
        Text {
          width: parent.width
          text: root.error
            ? root.error
            : (!(root.data && root.data.state)
              ? "Loading scene data…"
              : "No scenes configured")
          visible: root.error !== ""
            || !(root.data && root.data.state)
            || !(root.snapshot.scenes && root.snapshot.scenes.length)
          color: root.error ? "#f5b544" : Qt.alpha(root.foreground, 0.55)
          font.family: root.fontFamily
          font.pixelSize: 10
        }

        // ---- actions
        Row {
          width: parent.width
          spacing: Style.space(6)

          Button {
            text: root.paused ? "Resume" : "Pause 30m"
            bordered: true
            onClicked: root.paused ? root.resume() : root.pause()
          }
          Button {
            text: "Leave scene"
            visible: root.activeName !== "idle"
            bordered: true
            onClicked: root.leaveScene()
          }
          Button {
            text: "Edit config"
            bordered: true
            onClicked: root.editConfig()
          }
        }
      }
    }
  }
}
