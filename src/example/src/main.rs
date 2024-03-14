use std::backtrace::Backtrace;
use serial_thread::async_channel::{self, unbounded, Receiver, Sender};
use serial_thread::{Mode, SerialInterface, SerialMessage};
use serial_thread::serial::{Baud115200, BaudRate};

#[tokio::main]
async fn main() {
    let (app_sender, serial_receiver) = unbounded::<SerialMessage>();
    let (serial_sender, app_receiver) = unbounded::<SerialMessage>();

    let mut serial = SerialInterface::new()
        .unwrap()
        .sender(serial_sender)
        .receiver(serial_receiver.clone());

    // Spawn serial thread
    tokio::spawn(async move {
        serial.start().await;
    });

    // Select Serial port
    app_sender.send(SerialMessage::SetPort("/dev/ttyUSB0".to_string())).await.unwrap();

    // Select baud rate
    app_sender.send(SerialMessage::SetBauds(Baud115200)).await.unwrap();

    // Connect serial port
    app_sender.send(SerialMessage::Connect).await.unwrap();

    // Start Sniff mode
    app_sender.send(SerialMessage::SetMode(Mode::Sniff)).await.unwrap();

    loop {
        if let Ok(msg) = app_receiver.try_recv() {
            println!("{:?}", msg);
        }

    }


}
