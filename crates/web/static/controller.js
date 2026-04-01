class Controller {
  constructor(setStatus) {
    this.ws = null;
    this.id = Array.from(crypto.getRandomValues(new Uint8Array(3)));
    this.seq = 0;
    this.buttons = 0;
    this.xAxis = 0;
    this.yAxis = 0;
    this.setStatus = setStatus;
  }

  connect(layout) {
    if (this.ws) return;

    var scheme = location.protocol === "https:" ? "wss" : "ws";
    var url = scheme + "://" + location.host + "/ws";

    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.ws.send(
        JSON.stringify({
          Register: {
            id: this.id,
            layout,
          },
        }),
      );
      this.setStatus("Connected");
    };

    this.ws.onclose = () => {
      this.buttons = 0;
      this.axes = [];
      this.ws = null;
    };

    this.ws.onerror = () => {
      this.setStatus("Connection error");
    };

    // Server sends no messages
    this.ws.onmessage = () => {};
  }

  sendInput() {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.seq++;
    this.ws.send(
      JSON.stringify({
        Input: {
          id: this.id,
          seq: this.seq,
          buttons: this.buttons,
          x_axis: this.xAxis,
          y_axis: this.yAxis,
        },
      }),
    );
  }

  pressButton(bit) {
    if (this.buttons & bit) return; // already pressed
    this.buttons |= bit;
    this.sendInput();
  }

  releaseButton(bit) {
    if (!(this.buttons & bit)) return; // already released
    this.buttons &= ~bit;
    this.sendInput();
  }

  setAxes(x, y) {
    this.xAxis = x;
    this.yAxis = y;
    this.sendInput();
  }
}

export default Controller;
