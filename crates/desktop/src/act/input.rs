// Input capability — SendInput wrapper.
// Syntax: ^c (Ctrl+C), +{Tab} (Shift+Tab), %{F4} (Alt+F4), {Enter}, text.
// Modifiers stack: ^ Ctrl, + Shift, % Alt. Can combine: +^t = Ctrl+Shift+T.
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

const VK_CONTROL: u16 = 0x11;
const VK_SHIFT: u16 = 0x10;
const VK_MENU: u16 = 0x12; // Alt

pub fn send_keys(keys: &str) {
    let chars: Vec<char> = keys.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        i += send_one(&chars, i);
    }
}

fn send_one(chars: &[char], i: usize) -> usize {
    // Collect modifier prefixes
    let mut mods: Vec<u16> = Vec::new();
    let mut j = i;
    while j < chars.len() {
        match chars[j] {
            '^' => { mods.push(VK_CONTROL); j += 1; }
            '+' => { mods.push(VK_SHIFT); j += 1; }
            '%' => { mods.push(VK_MENU); j += 1; }
            _ => break,
        }
    }
    if j >= chars.len() { return j - i; }

    // Press modifiers
    for &vk in &mods { send_key(vk, 0); }

    match chars[j] {
        '{' => {
            if let Some(end) = chars[j..].iter().position(|&c| c == '}') {
                let name: String = chars[j+1..j+end].iter().collect();
                send_named(&name);
                for &vk in mods.iter().rev() { send_key(vk, KEYEVENTF_KEYUP); }
                return (j + end + 1) - i;
            }
        }
        c => {
            if !mods.is_empty() {
                // Use virtual key for modifier combos (Ctrl+W, Shift+T, etc.)
                let vk = c.to_ascii_uppercase() as u16;
                let inputs = [keybd(vk, 0), keybd(vk, KEYEVENTF_KEYUP)];
                send_batch(&inputs);
            } else {
                send_unicode(c);
            }
            for &vk in mods.iter().rev() { send_key(vk, KEYEVENTF_KEYUP); }
            return (j + 1) - i;
        }
    }
    j - i
}

fn send_named(name: &str) {
    let vk = match name {
        "Enter" => 0x0D, "Backspace" => 0x08, "Tab" => 0x09,
        "Escape" => 0x1B, "Space" => 0x20,
        "Left" => 0x25, "Up" => 0x26, "Right" => 0x27, "Down" => 0x28,
        "Delete" => 0x2E,
        "Home" => 0x24, "End" => 0x23,
        "F1"=>0x70, "F2"=>0x71, "F3"=>0x72, "F4"=>0x73, "F5"=>0x74,
        "F6"=>0x75, "F7"=>0x76, "F8"=>0x77, "F9"=>0x78, "F10"=>0x79,
        "F11"=>0x7A, "F12"=>0x7B,
        _ => return,
    };
    let inputs = [keybd(vk, 0), keybd(vk, KEYEVENTF_KEYUP)];
    send_batch(&inputs);
}

fn send_key(vk: u16, flags: u32) {
    send_batch(&[keybd(vk, flags)]);
}

fn send_unicode(c: char) {
    send_batch(&[unicode(c)]);
}

fn keybd(vk: u16, flags: u32) -> INPUT {
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

fn unicode(c: char) -> INPUT {
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0, wScan: c as u16, dwFlags: KEYEVENTF_UNICODE, time: 0, dwExtraInfo: 0 } } }
}

fn send_batch(inputs: &[INPUT]) {
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr() as *mut INPUT, std::mem::size_of::<INPUT>() as i32); }
}
