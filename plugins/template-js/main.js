ctx.log("info", "JS plugin started");

function on_mouse_down(event) {
    ctx.log("debug", "JS: down " + event.button + " at " + event.x + "," + event.y);
    if (event.button === "Right") {
        ctx.commit_intent("window", JSON.stringify({
            width: 200,
            height: 80,
            position: "near_cursor",
            auto_close: true,
            draws: [
                { type: "text", x: 10, y: 10, text: "JS popup!", font_size: 14, color: 0x333333 }
            ]
        }));
    }
}

function on_mouse_move(event) {
    ctx.log("debug", "JS: move at " + event.x + "," + event.y);
}

function on_mouse_up(event) {
    ctx.log("debug", "JS: up " + event.button + " at " + event.x + "," + event.y);
}
