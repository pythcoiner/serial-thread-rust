use serial_thread::async_channel::{self, unbounded, Receiver, Sender};
use serial_thread::{Mode, SerialInterface, SerialMessage};
use serial_thread::serial::{BaudRate};

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
    app_sender.send(SerialMessage::SetBauds(BaudRate::Baud115200)).await.unwrap();

    // Connect serial port
    app_sender.send(SerialMessage::Connect).await.unwrap();

    // Start Sniff mode
    app_sender.send(SerialMessage::SetMode(Mode::Sniff)).await.unwrap();

    // Listen for messages:
    // First message should be SerialMessage::Connected(<true/false>)
    // Then request of type SerialMessage::Receive([bytes])
    loop {
        if let Ok(msg) = app_receiver.try_recv() {
            println!("{:?}", msg);
        }

    }
}
