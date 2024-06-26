pub use tokio;
pub use serial;
use serial::{BaudRate, CharSize, FlowControl, Parity, SerialPort, StopBits, SystemPort};
use serialport::available_ports;
use std::io::{Read, Write};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[cfg(feature = "async-channel")]
pub use async_channel;
#[cfg(feature = "async-channel")]
use async_channel::{Receiver, Sender};
#[cfg(not(feature = "async-channel"))]
use std::sync::mpsc::{Receiver, Sender};


#[derive(Debug, Clone)]
pub enum SerialInterfaceError {
    CannotListPorts,
    StopToChangeSettings,
    DisconnectToChangeSettings,
    CannotReadPort(Option<String>),
    WrongReadArguments,
    CannotOpenPort(String),
    PortNotOpened,
    SlaveModeNeedModbusID,
    PortAlreadyOpen,
    PortNeededToOpenPort,
    SilenceMissing,
    PathMissing,
    NoPortToClose,
    CannotSendMessage,
    WrongMode,
    CannotWritePort,
    StopModeBeforeChange,
    WaitingForResponse,
    CannotSetTimeout,
}

/// Represents the status of the SerialInterface, indicating its current operation or state.
#[derive(Debug, Clone)]
pub enum Status {
    Read,
    Receipt,
    Write,
    WaitingResponse,
    None,
}

/// Defines the operating modes of the SerialInterface.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Operating as a master in a master-slave configuration.
    Master,
    /// Operating as a master in a master-slave configuration w/ stream reading mode (no silence,
    /// screening read instead)
    MasterStream,
    /// Operating as a slave in a master-slave configuration.
    Slave,
    /// Listening on the serial line without interfering.
    Sniff,
    /// Stopped or inactive state.
    Stop,
}

#[derive(Debug, Clone)]
pub enum SerialMessage {
    // Settings / Flow control (handled when Mode = Stop)

    /// Request: Lists available serial ports.
    /// Handled in 'Stop' mode. Response: Triggers `AvailablePorts` message with port list.
    ListPorts,

    /// Response: Provides a list of available serial ports.
    /// Type: Vec<String> representing port names.
    AvailablePorts(Vec<String>),

    /// Request: Sets the serial port to be used.
    /// Type: String representing the port path.
    /// Handled in 'Stop' mode. Affects settings for subsequent `Connect` commands.
    SetPort(String),

    /// Request: Sets the baud rate for the serial communication.
    /// Type: BaudRate.
    /// Handled in 'Stop' mode. Updates baud rate settings for the serial interface.
    SetBauds(BaudRate),

    /// Request: Sets the character size for the serial communication.
    /// Type: CharSize.
    /// Handled in 'Stop' mode. Updates character size settings for the serial interface.
    SetCharSize(CharSize),

    /// Request: Sets the parity for the serial communication.
    /// Type: Parity.
    /// Handled in 'Stop' mode. Updates parity settings for the serial interface.
    SetParity(Parity),
    /// Request: Sets the stop bits for the serial communication.
    /// Type: StopBits.
    /// Handled in 'Stop' mode. Updates stop bits settings for the serial interface.
    SetStopBits(StopBits),

    /// Request: Sets the flow control for the serial communication.
    /// Type: FlowControl.
    /// Handled in 'Stop' mode. Updates flow control settings for the serial interface.
    SetFlowControl(FlowControl),

    /// Request: Sets the timeout for the serial communication.
    /// Type: Duration.
    /// Handled in all modes. Updates timeout settings for the serial interface.
    SetTimeout(Duration),

    /// Request: Establishes a connection using the current serial port settings.
    /// Handled in 'Stop' mode. Response: `Connected(true)` on success, or an `Error` message on failure.
    Connect,

    /// Request: Disconnects the current serial connection.
    /// Handled in all modes. Response: `Connected(false)` after disconnection.
    Disconnect,

    // Data messages (handled when mode != Stop)

    /// Request: Sends data over the serial connection.
    /// Type: Vec<u8> representing the data to be sent.
    /// Handled when mode is not 'Stop'. Response: `DataSent` with the sent data upon successful transmission.
    Send(Vec<u8>),

    /// Response: Indicates that data has been sent over the serial connection.
    /// Type: Vec<u8> representing the sent data.
    DataSent(Vec<u8>),

    /// Response: Indicates received data over the serial connection.
    /// Type: Vec<u8> representing the received data.
    Receive(Vec<u8>),

    /// Response: Indicates that data has been sent over the serial connection but no response 
    /// from the peer.
    NoResponse,

    // General messages (always handled)

    /// Request: Retrieves the current status of the serial interface.
    /// Response: `Status` with the current status of the interface.
    GetStatus,

    /// Response: Indicates the current status of the serial interface.
    /// Type: Status enum.
    Status(Status),

    /// Request: Retrieves the current connection status of the serial interface.
    /// Response: `Connected` indicating whether the interface is connected.
    GetConnectionStatus,

    /// Response: Indicates the current connection status of the serial interface.
    /// Type: bool indicating connection status.
    Connected(bool),

    /// Request: Sets the operating mode of the SerialInterface.
    /// Type: Mode enum.
    /// Changes the operating mode of the interface.
    SetMode(Mode),

    /// Response: Indicates the current operating mode of the SerialInterface.
    /// Type: Mode enum.
    Mode(Mode),

    /// Response: Represents an error within the SerialInterface.
    /// Type: SIError enum.
    Error(SIError),

    /// Request: Ping message for connection testing.
    /// Response: Generates a `Pong` message in response.
    Ping,

    /// Response: Pong message as a response to a Ping.
    Pong,
}

type SIError = SerialInterfaceError;

/// Represents a serial interface with various modes and functionalities.
/// It handles serial communication, including reading, writing, and managing port settings.
/// It operates in different modes such as Master, Slave, and Sniff.
pub struct SerialInterface {
    path: Option<String>,
    mode: Mode,
    status: Status,
    modbus_id: Option<u8>,
    baud_rate: BaudRate,
    char_size: CharSize,
    parity: Parity,
    stop_bits: StopBits,
    flow_control: FlowControl,
    port: Option<SystemPort>,
    silence: Option<Duration>,
    timeout: Duration,
    receiver: Option<Receiver<SerialMessage>>,
    sender: Option<Sender<SerialMessage>>,
    last_byte_time: Option<Instant>,
}

impl SerialInterface {
    /// Creates a new instance of the SerialInterface with default settings.
    /// Returns a SerialInterface object encapsulated in a Result, with an error if initialization fails.
    pub fn new() -> Result<Self, SIError> {
        Ok(SerialInterface {
            path: None,
            mode: Mode::Stop,
            status: Status::None,
            modbus_id: None,
            baud_rate: BaudRate::Baud115200,
            char_size: CharSize::Bits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::Stop2,
            flow_control: FlowControl::FlowNone,
            port: None,
            silence: Some(Duration::from_nanos(800)), // FIXME: what policy for init silence here?
            timeout: Duration::from_nanos(10000),     // FIXME: what policy for init timeout here?
            receiver: None,
            sender: None,
            last_byte_time: None,
        })
    }

    /// Sets the path for the serial interface.
    /// Returns the modified instance of the SerialInterface for method chaining.
    pub fn path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    /// Sets the baud rate for the serial interface.
    /// Returns the modified instance of the SerialInterface for method chaining.
    pub fn bauds(mut self, bauds: BaudRate) -> Self {
        self.baud_rate = bauds;
        // TODO: if self.silence is none => automatic choice
        self
    }

    /// Sets the character size for the serial interface.
    /// Returns the modified instance of the SerialInterface for method chaining.
    pub fn char_size(mut self, size: CharSize) -> Self {
        self.char_size = size;
        self
    }

    /// Sets the parity for the serial interface.
    /// Returns the modified instance of the SerialInterface for method chaining.
    pub fn parity(mut self, parity: Parity) -> Self {
        self.parity = parity;
        self
    }

    /// Sets the parity for the serial interface.
    /// Returns the modified instance of the SerialInterface for method chaining.
    pub fn stop_bits(mut self, stop_bits: StopBits) -> Self {
        self.stop_bits = stop_bits;
        self
    }

    /// Sets the flow control for the serial interface.
    /// Returns the modified instance of the SerialInterface for method chaining.
    pub fn flow_control(mut self, flow_control: FlowControl) -> Self {
        self.flow_control = flow_control;
        self
    }

    /// Sets the Modbus ID for the serial interface.
    /// Returns the modified instance of the SerialInterface for method chaining.
    pub fn modbus_id(mut self, modbus_id: u8) -> Self {
        self.modbus_id = Some(modbus_id);
        self
    }

    /// Sets the silence interval for the serial interface. Silence interval used to detect
    /// end of modbus frame.
    /// Returns the modified instance of the SerialInterface for method chaining.
    pub fn silence(mut self, silence: Duration) -> Self {
        self.silence = Some(silence);
        self
    }

    /// Sets the receiver channel for the serial interface.
    /// Returns the modified instance of the SerialInterface for method chaining.
    pub fn receiver(mut self, receiver: Receiver<SerialMessage>) -> Self {
        self.receiver = Some(receiver);
        self
    }

    /// Sets the sender channel for the serial interface.
    /// Returns the modified instance of the SerialInterface for method chaining.
    pub fn sender(mut self, sender: Sender<SerialMessage>) -> Self {
        self.sender = Some(sender);
        self
    }

    /// Sets the operating mode of the SerialInterface.
    /// Can only be set when the current mode is 'Stop'.
    /// Returns a Result with () or an error if the mode cannot be changed.
    pub fn set_mode(&mut self, m: Mode) -> Result<(), SIError> {
        if let Mode::Stop = &self.mode {
            if self.modbus_id.is_none() {
                if let Mode::Slave = m {
                    return Err(SIError::SlaveModeNeedModbusID);
                }
            } else if self.port.is_some() {
                return Err(SIError::DisconnectToChangeSettings);
            }
            self.mode = m;
            log::info!("SerialInterface::switch mode to {:?}", &self.mode);
            Ok(())
        } else {
            Err(SIError::StopToChangeSettings)
        }
    }

    /// Retrieves the current operating mode of the SerialInterface.
    pub fn get_mode(&self) -> &Mode {
        &self.mode
    }

    /// Retrieves the current status of the SerialInterface.
    pub fn get_state(&self) -> &Status {
        &self.status
    }

    /// Lists available serial ports.
    /// Returns a Result containing a list of port names or an error if ports cannot be listed.
    pub fn list_ports() -> Result<Vec<String>, SIError> {
        // TODO: get rid of serialport crate dependency
        if let Ok(ports) = available_ports() {
            Ok(ports.iter().map(|p| p.port_name.clone()).collect())
        } else {
            Err(SerialInterfaceError::CannotListPorts)
        }
        // Ok(vec!["/dev/ttyXR0".to_string(), "/dev/ttyXR1".to_string()])
    }

    /// CLear data from the read buffer.
    fn clear_read_buffer(&mut self) -> Result<(), SIError> {
        let port_open = self.port.is_some();
        if port_open {
            let mut buffer = [0u8; 24];
            loop {
                let read = self.port.as_mut().unwrap().read(&mut buffer);
                let ret = match read {
                    Ok(r) => {
                        log::debug!("SerialInterface::buffer clear {:?}", buffer.to_vec());
                        r
                    }
                    Err(e) => {
                        let str_err = e.to_string();
                        if str_err == *"Operation timed out" {
                            0
                        } else {
                            return Err(SIError::CannotReadPort(Some(str_err)));
                        }
                    }
                };
                if ret == 0 {
                    break;
                };
            }
            Ok(())
        } else {
            Err(SIError::PortNotOpened)
        }
    }

    /// Read 1 bytes of data, return None if no data in buffer.
    fn read_byte(&mut self) -> Result<Option<u8>, SIError> {
        let port_open = self.port.is_some();
        if port_open {
            let mut buffer = [0u8; 1];
            let read = self.port.as_mut().unwrap().read(&mut buffer);
            let l = match read {
                Ok(r) => r,
                Err(e) => {
                    let str_err = e.to_string();
                    if str_err == *"Operation timed out" {
                        0
                    } else {
                        return Err(SIError::CannotReadPort(Some(str_err)));
                    }
                }
            };
            if l > 0 {
                let rcv_time = Instant::now();
                let from_last = self
                    .last_byte_time
                    .map(|last_byte| rcv_time.duration_since(last_byte));
                log::debug!(
                    "SerialInterface::read_byte({:?}, from last: {:?})",
                    buffer,
                    from_last
                );
                self.last_byte_time = Some(rcv_time);
                Ok(Some(buffer[0]))
            } else {
                Ok(None)
            }
        } else {
            Err(SIError::PortNotOpened)
        }
    }
    
    /// Generalist read() implementation, polling serial buffer, while not data been received on serial buffer,
    /// checking received messages on self.receiver , if Send() received, return.
    /// Error if none of size/silence/timeout passed.
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    fn read_until_size_or_silence_or_timeout_or_message(
        &mut self,
        size: Option<usize>,
        silence: Option<&Duration>,
        timeout: Option<&Duration>,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.clear_read_buffer()?;
        let mut buffer: Vec<u8> = Vec::new();
        let start = Instant::now();
        let mut last_data = Instant::now();

        if !(size.is_some() || timeout.is_some() || silence.is_some()) {
            return Err(SIError::WrongReadArguments);
        }

        loop {
            let result = self.read_byte()?;
            // receive data
            if let Some(data) = result {
                // log::debug!("Start receive data: {}", data);
                self.status = Status::Receipt;
                buffer.push(data);
                // reset the silence counter
                last_data = Instant::now();

                // check for size reach
                if let Some(size) = &size {
                    if &buffer.len() == size {
                        let result = self
                            .send_message(SerialMessage::Receive(buffer.clone()));
                        self.status = Status::None;
                        return if let Err(e) = result {
                            Err(e)
                        } else {
                            Ok(None)
                        };
                    }
                }
            } else if let Some(silence) = silence {
                // we not yet start receive
                if buffer.is_empty() {
                    // Wait to receive first data
                    if let Some(msg) = self.read_message()? {
                        return Ok(Some(msg));
                    }
                    last_data = Instant::now();
                } else {
                    // receiving and waiting for silence
                    let from_last_data = &Instant::now().duration_since(last_data);
                    // log::debug!("Duration from last data: {:?}", from_last_data);
                    if from_last_data > silence {
                        log::debug!("silence reached, data received: {:?}", buffer.to_vec());
                        let result = self
                            .send_message(SerialMessage::Receive(buffer.clone()));
                        self.status = Status::None;
                        return if let Err(e) = result {
                            Err(e)
                        } else {
                            Ok(None)
                        };
                    }
                }
            }
            // check timeout
            if let Some(timeout) = timeout {
                if &Instant::now().duration_since(start) > timeout {
                    return if !buffer.is_empty() {
                        let result = self
                            .send_message(SerialMessage::Receive(buffer.clone()));
                        self.status = Status::None;
                        if let Err(e) = result {
                            Err(e)
                        } else {
                            Ok(None)
                        }
                    } else {
                        let result = self
                            .send_message(SerialMessage::NoResponse);
                        self.status = Status::None;
                        if let Err(e) = result {
                            Err(e)
                        } else {
                            Ok(None)
                        }
                    }
                }
            }
        }
    }

    /// Generalist read() implementation, polling serial buffer, while not data been received on serial buffer,
    /// checking received messages on self.receiver , if Send() received, return.
    /// Error if none of size/silence/timeout passed.
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    async fn read_until_size_or_silence_or_timeout_or_message(
        &mut self,
        size: Option<usize>,
        silence: Option<&Duration>,
        timeout: Option<&Duration>,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.clear_read_buffer()?;
        let mut buffer: Vec<u8> = Vec::new();
        let start = Instant::now();
        let mut last_data = Instant::now();

        if !(size.is_some() || timeout.is_some() || silence.is_some()) {
            return Err(SIError::WrongReadArguments);
        }

        loop {
            let result = self.read_byte()?;
            // receive data
            if let Some(data) = result {
                // log::debug!("Start receive data: {}", data);
                self.status = Status::Receipt;
                buffer.push(data);
                // reset the silence counter
                last_data = Instant::now();

                // check for size reach
                if let Some(size) = &size {
                    if &buffer.len() == size {
                        let result = self
                            .send_message(SerialMessage::Receive(buffer.clone()))
                            .await;
                        self.status = Status::None;
                        return if let Err(e) = result {
                            Err(e)
                        } else {
                            Ok(None)
                        };
                    }
                }
            } else if let Some(silence) = silence {
                // we not yet start receive
                if buffer.is_empty() {
                    // Wait to receive first data
                    if let Some(msg) = self.read_message().await? {
                        return Ok(Some(msg));
                    }
                    last_data = Instant::now();
                } else {
                    // receiving and waiting for silence
                    let from_last_data = &Instant::now().duration_since(last_data);
                    // log::debug!("Duration from last data: {:?}", from_last_data);
                    if from_last_data > silence {
                        log::debug!("silence reached, data received: {:?}", buffer.to_vec());
                        let result = self
                            .send_message(SerialMessage::Receive(buffer.clone()))
                            .await;
                        self.status = Status::None;
                        return if let Err(e) = result {
                            Err(e)
                        } else {
                            Ok(None)
                        };
                    }
                }
            }
            // check timeout
            if let Some(timeout) = timeout {
                if &Instant::now().duration_since(start) > timeout {
                    return if !buffer.is_empty() {
                        let result = self
                            .send_message(SerialMessage::Receive(buffer.clone()))
                            .await;
                        self.status = Status::None;
                        if let Err(e) = result {
                            Err(e)
                        } else {
                            Ok(None)
                        }
                    } else {
                        let result = self
                            .send_message(SerialMessage::NoResponse)
                            .await;
                        self.status = Status::None;
                        if let Err(e) = result {
                            Err(e)
                        } else {
                            Ok(None)
                        }
                    }
                }
            }
        }
    }

    pub fn crc16(data: &[u8]) -> u16 {
        let mut crc = 0xFFFF;
        for x in data {
            crc ^= u16::from(*x);
            for _ in 0..8 {
                // if we followed clippy's suggestion to move out the crc >>= 1, the condition may not be met any more
                // the recommended action therefore makes no sense and it is better to allow this lint
                #[allow(clippy::branches_sharing_code)]
                if (crc & 0x0001) != 0 {
                    crc >>= 1;
                    crc ^= 0xA001;
                } else {
                    crc >>= 1;
                }
            }
        }
        crc << 8 | crc >> 8
    }

    fn check_crc(frame: &[u8]) -> bool {
        // log::debug!("check_crc({:?})", frame);
        if frame.len() > 4 {
            let crc = Self::crc16(&frame[..frame.len()-2]);
            let expected_crc = [((crc & 0xff00) >> 8) as u8, (crc & 0x00ff) as u8];
            // log::debug!("expected crc: {:?}, end_of_frame: {:?}", &expected_crc, &frame[frame.len()-2..]);
            expected_crc == frame[frame.len()-2..]
        } else {
            false
        }

    }

    fn try_decode_buffer(buffer: Vec<u8>) -> Option<Vec<u8>> {
        let mut window_size = 5;

        while window_size <= buffer.len() {
            for i in 0..=buffer.len() - window_size {
                // Forward direction
                if Self::check_crc(&buffer[i..i + window_size]) {
                    return Some(buffer[i..i + window_size].to_vec());
                }
                
                if buffer.len() == window_size {
                    return None;
                }
                // Reverse direction
                let j = buffer.len() - i - window_size;
                if Self::check_crc(&buffer[j..j + window_size]) {
                    return Some(buffer[j..j + window_size].to_vec());
                }
            }
            window_size += 1;
        }
        None
    }


    /// Stream read() implementation, buffering the read data, and `screening` until we find 
    /// a frame w/ valid CRC
    #[allow(unused)]
    fn read_stream(&mut self, timeout: &Duration) -> Result<SerialMessage, SIError> {
        self.clear_read_buffer()?;
        let mut buffer: Vec<u8> = Vec::new();
        let start = Instant::now();

        loop {
            let result = self.read_byte()?;
            // receive data
            if let Some(data) = result {
                // log::debug!("Start receive data: {}", data);
                self.status = Status::Receipt;
                buffer.push(data);
                let decoded = Self::try_decode_buffer(buffer.clone());
                // log::debug!("try_decode_buffer({:?}) = {:?}", &buffer, decoded);
                if let Some(frame) = decoded {
                    return Ok(SerialMessage::Receive(frame));
                }
            }
            // check timeout
            if &Instant::now().duration_since(start) > timeout {
                return Ok(SerialMessage::NoResponse);
            }
            
        }
    }
        
    
    /// Read <s> bytes of data, blocking until get the <s> number of bytes.
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    fn read_size(&mut self, s: usize) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(Some(s), None, None)
    }

    /// Read <s> bytes of data, blocking until get the <s> number of bytes.
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    async fn read_size(&mut self, s: usize) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(Some(s), None, None)
            .await
    }

    
    
    /// Should be use to listen to a Request response in Master.
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    fn read_until_size_or_silence(
        &mut self,
        size: usize,
        silence: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(Some(size), Some(silence), None)
    }

    /// Should be use to listen to a Request response in Master.
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    async fn read_until_size_or_silence(
        &mut self,
        size: usize,
        silence: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(Some(size), Some(silence), None)
            .await
    }
    
    
    /// Should be use to listen in Slave/Sniffing , when you don't know the size of the incoming Request.
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    fn read_until_silence(
        &mut self,
        silence: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(None, Some(silence), None)
    }

    /// Should be use to listen in Slave/Sniffing , when you don't know the size of the incoming Request.
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    async fn read_until_silence(
        &mut self,
        silence: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(None, Some(silence), None)
            .await
    }

    
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    fn read_until_silence_or_timeout(
        &mut self,
        silence: &Duration,
        timeout: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(None, Some(silence), Some(timeout))
    }

    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    async fn read_until_silence_or_timeout(
        &mut self,
        silence: &Duration,
        timeout: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        self.read_until_size_or_silence_or_timeout_or_message(None, Some(silence), Some(timeout))
            .await
    }

    
    /// Open the serial port.
    pub fn open(&mut self) -> Result<(), SIError> {
        if self.port.is_some() || self.mode != Mode::Stop {
            Err(SIError::PortAlreadyOpen)
            // TODO => SlaveModeNeedModbusID move this Mode::Slave
            // } else if self.modbus_id.is_none() {
            //     Err(SIError::SlaveModeNeedModbusID)
            // } else if self.mode != Mode::Master && self.silence.is_none() {
            //     Err(SIError::SilenceMissing)
        } else if self.path.is_none() {
            Err(SIError::PathMissing)
        } else {
            let mut port = serial::open(&self.path.as_ref().unwrap())
                .map_err(|e| SIError::CannotOpenPort(e.to_string()))?;
            let settings = serial::PortSettings {
                baud_rate: self.baud_rate,
                char_size: self.char_size,
                parity: self.parity,
                stop_bits: self.stop_bits,
                flow_control: self.flow_control,
            };
            port.configure(&settings).unwrap();
            port.set_timeout(Duration::from_nanos(10))
                .map_err(|_| SIError::CannotSetTimeout)?;
            self.port = Some(port);
            Ok(())
        }
    }

    /// Close the serial port.
    pub fn close(&mut self) -> Result<(), SIError> {
        if let Some(port) = self.port.take() {
            drop(port);
            Ok(())
        } else {
            Err(SIError::NoPortToClose)
        }
    }
    

    /// Try to send a message trough self.sender
    #[cfg(not(feature = "async-channel"))]
    fn send_message(&mut self, msg: SerialMessage) -> Result<(), SIError> {
        log::debug!("SerialInterface.send_message({:?})", msg);
        if let Some(sender) = self.sender.clone() {
            log::debug!("SerialInterface::Send {:?}", &msg);
            sender
                .send(msg)
                .map_err(|_| SIError::CannotSendMessage)?;
            Ok(())
        } else {
            log::debug!("SerialInterface::SIError::CannotSendMessage");
            Err(SIError::CannotSendMessage)
        }
    }

    /// Try to send a message trough self.sender
    #[cfg(feature = "async-channel")]
    async fn send_message(&mut self, msg: SerialMessage) -> Result<(), SIError> {
        if let Some(sender) = self.sender.clone() {
            log::debug!("SerialInterface::Send {:?}", &msg);
            sender
                .send(msg)
                .await
                .map_err(|_| SIError::CannotSendMessage)?;
            Ok(())
        } else {
            log::debug!("SerialInterface::SIError::CannotSendMessage");
            Err(SIError::CannotSendMessage)
        }
    }
    

    /// Poll self.receiver channel and handle if there is one message. Return the message if it should be
    /// handled externally. Two kind messages can be returned:
    /// - SerialMessage::SetMode()
    /// - SerialMessage::Send()
    #[cfg(not(feature = "async-channel"))]
    fn read_message(&mut self) -> Result<Option<SerialMessage>, SIError> {
        if let Some(receiver) = &mut self.receiver {
            if let Ok(message) = receiver.try_recv() {
                log::debug!("SerialInterface::read_message({:?})", &message);
                // general case, message to handle in any situation
                match &message {
                    SerialMessage::GetConnectionStatus => {
                        if let Some(_port) = &self.port {
                            self.send_message(SerialMessage::Connected(true))?;
                        } else {
                            self.send_message(SerialMessage::Connected(false))?;
                        }
                        return Ok(None);
                    }
                    SerialMessage::GetStatus => {
                        self.send_message(SerialMessage::Status(self.status.clone()))?;
                        return Ok(None);
                    }
                    // If ask for change mode, we return message to caller in order it can handle it.
                    SerialMessage::SetMode(mode) => {
                        return Ok(Some(SerialMessage::SetMode(mode.clone())));
                    }
                    SerialMessage::SetTimeout(timeout) => {
                        self.timeout = *timeout;
                        return Ok(None);
                    }
                    SerialMessage::Ping => {
                        self.send_message(SerialMessage::Pong)?;
                        return Ok(None);
                    }
                    _ => {}
                }

                // Stop case: Settings / Flow control
                if self.mode == Mode::Stop {
                    match message {
                        SerialMessage::ListPorts => {
                            self.send_message(SerialMessage::AvailablePorts(
                                SerialInterface::list_ports()?,
                            ))?;
                            return Ok(None);
                        }
                        SerialMessage::SetPort(port) => {
                            self.path = Some(port);
                            return Ok(None);
                        }
                        SerialMessage::SetBauds(bauds) => {
                            self.baud_rate = bauds;
                            // TODO: update silence?
                            return Ok(None);
                        }
                        SerialMessage::SetCharSize(char_size) => {
                            self.char_size = char_size;
                            return Ok(None);
                        }
                        SerialMessage::SetParity(parity) => {
                            self.parity = parity;
                            return Ok(None);
                        }
                        SerialMessage::SetStopBits(stop_bits) => {
                            self.stop_bits = stop_bits;
                            return Ok(None);
                        }
                        SerialMessage::SetFlowControl(flow_control) => {
                            self.flow_control = flow_control;
                            return Ok(None);
                        }
                        SerialMessage::Connect => {
                            if let Err(e) = self.open() {
                                log::debug!("Connect::{:?}", e);
                                self.send_message(SerialMessage::Connected(false))?;
                                self.send_message(SerialMessage::Error(e))?;
                            } else {
                                self.send_message(SerialMessage::Connected(true))?;
                            }
                            return Ok(None);
                        }
                        SerialMessage::Disconnect => {
                            let result = self.close();
                            self.send_message(SerialMessage::Connected(false))?;
                            if let Err(e) = result {
                                self.send_message(SerialMessage::Error(e))?;
                            }
                        }
                        _ => {}
                    }
                } else if let SerialMessage::Send(data) = message {
                    return Ok(Some(SerialMessage::Send(data)));
                }
            }
        } else {
            log::debug!("No receiver!");
        }
        Ok(None)
    }

    /// Poll self.receiver channel and handle if there is one message. Return the message if it should be
    /// handled externally. Two kind messages can be returned:
    /// - SerialMessage::SetMode()
    /// - SerialMessage::Send()
    #[cfg(feature = "async-channel")]
    async fn read_message(&mut self) -> Result<Option<SerialMessage>, SIError> {
        if let Some(receiver) = self.receiver.clone() {
            if let Ok(message) = receiver.try_recv() {
                log::debug!("SerialInterface::Receive !!! {:?}", &message);
                // general case, message to handle in any situation
                match &message {
                    SerialMessage::GetConnectionStatus => {
                        if let Some(_port) = &self.port {
                            self.send_message(SerialMessage::Connected(true)).await?;
                        } else {
                            self.send_message(SerialMessage::Connected(false)).await?;
                        }
                        return Ok(None);
                    }
                    SerialMessage::GetStatus => {
                        self.send_message(SerialMessage::Status(self.status.clone()))
                            .await?;
                        return Ok(None);
                    }
                    // If ask for change mode, we return message to caller in order it can handle it.
                    SerialMessage::SetMode(mode) => {
                        return Ok(Some(SerialMessage::SetMode(mode.clone())));
                    }
                    SerialMessage::SetTimeout(timeout) => {
                        self.timeout = *timeout;
                        return Ok(None);
                    }
                    SerialMessage::Ping => {
                        self.send_message(SerialMessage::Pong).await?;
                        return Ok(None);
                    }
                    _ => {}
                }

                // Stop case: Settings / Flow control
                if self.mode == Mode::Stop {
                    match message {
                        SerialMessage::ListPorts => {
                            self.send_message(SerialMessage::AvailablePorts(
                                SerialInterface::list_ports()?,
                            ))
                                .await?;
                            return Ok(None);
                        }
                        SerialMessage::SetPort(port) => {
                            self.path = Some(port);
                            return Ok(None);
                        }
                        SerialMessage::SetBauds(bauds) => {
                            self.baud_rate = bauds;
                            // TODO: update silence?
                            return Ok(None);
                        }
                        SerialMessage::SetCharSize(char_size) => {
                            self.char_size = char_size;
                            return Ok(None);
                        }
                        SerialMessage::SetParity(parity) => {
                            self.parity = parity;
                            return Ok(None);
                        }
                        SerialMessage::SetStopBits(stop_bits) => {
                            self.stop_bits = stop_bits;
                            return Ok(None);
                        }
                        SerialMessage::SetFlowControl(flow_control) => {
                            self.flow_control = flow_control;
                            return Ok(None);
                        }
                        SerialMessage::Connect => {
                            if let Err(e) = self.open() {
                                log::debug!("Connect::{:?}", e);
                                self.send_message(SerialMessage::Connected(false)).await?;
                                self.send_message(SerialMessage::Error(e)).await?;
                            } else {
                                self.send_message(SerialMessage::Connected(true)).await?;
                            }
                            return Ok(None);
                        }
                        SerialMessage::Disconnect => {
                            let result = self.close();
                            self.send_message(SerialMessage::Connected(false)).await?;
                            if let Err(e) = result {
                                self.send_message(SerialMessage::Error(e)).await?;
                            }
                        }
                        _ => {}
                    }
                } else if let SerialMessage::Send(data) = message {
                    return Ok(Some(SerialMessage::Send(data)));
                }
            }
        }  else {
            log::debug!("No receiver!");
        }
        Ok(None)
    }

    
    /// Write data to the serial line.
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    fn write(&mut self, data: Vec<u8>) -> Result<(), SIError> {
        log::debug!("write({:?})", data.clone());
        let port_open = self.port.is_some();
        if port_open {
            let buffer = &data[0..data.len()];
            self.port
                .as_mut()
                .unwrap()
                .write(buffer)
                .map_err(|_| SIError::CannotWritePort)?;
            self.send_message(SerialMessage::DataSent(data))?;
            Ok(())
        } else {
            Err(SIError::PortNotOpened)
        }
    }

    /// Write data to the serial line.
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    async fn write(&mut self, data: Vec<u8>) -> Result<(), SIError> {
        log::debug!("write({:?})", data.clone());
        let port_open = self.port.is_some();
        if port_open {
            let buffer = &data[0..data.len()];
            self.port
                .as_mut()
                .unwrap()
                .write(buffer)
                .map_err(|_| SIError::CannotWritePort)?;
            self.send_message(SerialMessage::DataSent(data)).await?;
            Ok(())
        } else {
            Err(SIError::PortNotOpened)
        }
    }
    
    
    /// Sniffing feature: listen on serial line and send a SerialMessage::Receive() via mpsc channel for every serial
    /// request received, for every loop iteration, check if a SerialMessage is arrived via mpsc channel.
    /// If receive a SerialMessage::Send(), pause listen in order to send message then resume listening.
    /// Stop listening if receive SerialMessage::SetMode(Stop). Almost SerialMessage are handled silently by self.read_message().
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    pub fn listen(&mut self) -> Result<Option<Mode>, SIError> {
        loop {
            if let Some(silence) = &self.silence.clone() {
                // log::debug!("silence={:?}", silence);
                self.status = Status::Read;
                if let Some(msg) = self.read_until_silence(silence)? {
                    match msg {
                        SerialMessage::Send(data) => {
                            self.status = Status::Write;
                            let write = self.write(data);
                            self.status = Status::None;
                            if let Err(e) = write {
                                self.send_message(SerialMessage::Error(e))?;
                            }
                        }
                        SerialMessage::SetMode(mode) => {
                            if mode != Mode::Stop && mode != Mode::Sniff {
                                self.send_message(SerialMessage::Error(
                                    SIError::StopModeBeforeChange,
                                ))?;
                            } else if let Mode::Stop = mode {
                                self.status = Status::None;
                                return Ok(Some(mode));
                            }
                        }
                        _ => {}
                    }
                } else {
                    self.status = Status::None;
                    return Ok(None);
                }
            } else {
                return Err(SIError::SilenceMissing);
            }
        }
    }

    /// Sniffing feature: listen on serial line and send a SerialMessage::Receive() via mpsc channel for every serial
    /// request received, for every loop iteration, check if a SerialMessage is arrived via mpsc channel.
    /// If receive a SerialMessage::Send(), pause listen in order to send message then resume listening.
    /// Stop listening if receive SerialMessage::SetMode(Stop). Almost SerialMessage are handled silently by self.read_message().
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    pub async fn listen(&mut self) -> Result<Option<Mode>, SIError> {
        loop {
            if let Some(silence) = &self.silence.clone() {
                log::debug!("silence={:?}", silence);
                self.status = Status::Read;
                if let Some(msg) = self.read_until_silence(silence).await? {
                    match msg {
                        SerialMessage::Send(data) => {
                            self.status = Status::Write;
                            let write = self.write(data).await;
                            self.status = Status::None;
                            if let Err(e) = write {
                                self.send_message(SerialMessage::Error(e)).await?;
                            }
                        }
                        SerialMessage::SetMode(mode) => {
                            if mode != Mode::Stop && mode != Mode::Sniff {
                                self.send_message(SerialMessage::Error(
                                    SIError::StopModeBeforeChange,
                                ))
                                    .await?;
                            } else if let Mode::Stop = mode {
                                self.status = Status::None;
                                return Ok(Some(mode));
                            }
                        }
                        _ => {}
                    }
                } else {
                    self.status = Status::None;
                    return Ok(None);
                }
            } else {
                return Err(SIError::SilenceMissing);
            }
        }
    }

    
    /// Master feature: write a request, then wait for response, when response received, stop listening.
    /// Returns early if receive SerialMessage::SetMode(Mode::Stop)). Does not accept SerialMessage::Send() as
    /// we already waiting for a response. Almost SerialMessage are handled silently by self.read_message().
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    pub fn write_read(
        &mut self,
        data: Vec<u8>,
        timeout: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        if let Some(silence) = &self.silence.clone() {
            self.status = Status::Write;
            if let Err(e) = self.write(data) {
                self.status = Status::None;
                return Err(e);
            } else {
                self.status = Status::WaitingResponse;
            }

            loop {
                if let Some(msg) = self.read_until_silence_or_timeout(silence, timeout)? {
                    match msg {
                        SerialMessage::Send(_data) => {
                            // we already waiting for response cannot send request now.
                            self.send_message(SerialMessage::Error(SIError::WaitingForResponse))?;
                            continue;
                        }
                        SerialMessage::SetMode(mode) => {
                            if mode == Mode::Stop {
                                self.status = Status::None;
                                return Ok(Some(SerialMessage::SetMode(Mode::Stop)));
                            } else if mode == Mode::Slave || mode == Mode::Sniff {
                                self.send_message(SerialMessage::Error(
                                    SIError::StopModeBeforeChange,
                                ))?;
                                continue;
                            }
                        }
                        _ => {
                            continue;
                        }
                    }
                } else {
                    // Stop after silence or timeout, return
                    self.status = Status::None;
                    return Ok(None);
                }
            }
        } else {
            Err(SIError::SilenceMissing)
        }
    }

    /// Master feature: write a request, then wait for response, when response received, stop listening.
    /// Returns early if receive SerialMessage::SetMode(Mode::Stop)). Does not accept SerialMessage::Send() as
    /// we already waiting for a response. Almost SerialMessage are handled silently by self.read_message().
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    pub async fn write_read(
        &mut self,
        data: Vec<u8>,
        timeout: &Duration,
    ) -> Result<Option<SerialMessage>, SIError> {
        if let Some(silence) = &self.silence.clone() {
            self.status = Status::Write;
            if let Err(e) = self.write(data).await {
                self.status = Status::None;
                return Err(e);
            } else {
                self.status = Status::WaitingResponse;
            }

            loop {
                if let Some(msg) = self.read_until_silence_or_timeout(silence, timeout).await? {
                    match msg {
                        SerialMessage::Send(_data) => {
                            // we already waiting for response cannot send request now.
                            self.send_message(SerialMessage::Error(SIError::WaitingForResponse))
                                .await?;
                            continue;
                        }
                        SerialMessage::SetMode(mode) => {
                            if mode == Mode::Stop {
                                self.status = Status::None;
                                return Ok(Some(SerialMessage::SetMode(Mode::Stop)));
                            } else if mode == Mode::Slave || mode == Mode::Sniff {
                                self.send_message(SerialMessage::Error(
                                    SIError::StopModeBeforeChange,
                                ))
                                    .await?;
                                continue;
                            }
                        }
                        _ => {
                            continue;
                        }
                    }
                } else {
                    // Stop after silence or timeout, return
                    self.status = Status::None;
                    return Ok(None);
                }
            }
        } else {
            Err(SIError::SilenceMissing)
        }
    }


    /// Master stream feature: write a request, then wait for response in stream read mode, when response received, stop listening.
    /// Returns early if receive SerialMessage::SetMode(Mode::Stop)). Does not accept SerialMessage::Send() as
    /// we already waiting for a response. Almost SerialMessage are handled silently by self.read_message().
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    pub fn write_read_stream(
        &mut self,
        data: Vec<u8>,
        timeout: &Duration,
    ) -> Result<(), SIError> {

        self.status = Status::Write;
        if let Err(e) = self.write(data) {
            self.status = Status::None;
            return Err(e);
        } else {
            self.status = Status::WaitingResponse;
        }
        match self.read_stream(timeout) {
            Ok(msg) => {
                self.send_message(msg);
                self.status = Status::None;
                Ok(())
            }
            Err(e) => {
                self.status = Status::None;
                Err(e)
            }
        }
    }

    /// Master stream feature: write a request, then wait for response in stream read mode, when response received, stop listening.
    /// Returns early if receive SerialMessage::SetMode(Mode::Stop)). Does not accept SerialMessage::Send() as
    /// we already waiting for a response. Almost SerialMessage are handled silently by self.read_message().
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    pub async  fn write_read_stream(
        &mut self,
        data: Vec<u8>,
        timeout: &Duration,
    ) -> Result<(), SIError> {

        self.status = Status::Write;
        if let Err(e) = self.write(data).await {
            self.status = Status::None;
            return Err(e);
        } else {
            self.status = Status::WaitingResponse;
        }
        match self.read_stream(timeout) {
            Ok(msg) => {
                self.send_message(msg);
                self.status = Status::None;
                Ok(())
            }
            Err(e) => {
                self.status = Status::None;
                Err(e)
            }
        }
    }

    
    #[cfg(not(feature = "async-channel"))]
    /// Slave feature: listen the line until request receive, then stop listening. Returns early if receive
    /// SerialMessage::SetMode(Mode::Stop) or SerialMessage::Send(). Almost SerialMessage are handled silently
    /// by self.read_message().
    #[allow(unused)]
    pub fn wait_for_request(&mut self) -> Result<Option<SerialMessage>, SIError> {
        if let Some(silence) = self.silence {
            loop {
                self.status = Status::Read;
                let result = self.read_until_silence(&silence);
                self.status = Status::None;
                let read: Option<SerialMessage> = match result {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(e);
                    }
                };
                if let Some(msg) = read {
                    match msg {
                        SerialMessage::Send(data) => {
                            return Ok(Some(SerialMessage::Send(data.clone())));
                        }
                        SerialMessage::SetMode(mode) => {
                            if mode == Mode::Stop {
                                return Ok(Some(SerialMessage::SetMode(Mode::Stop)));
                            } else {
                                self.send_message(SerialMessage::Error(
                                    SIError::StopModeBeforeChange,
                                ))?;
                                continue;
                            }
                        }
                        _ => {
                            continue;
                        }
                    }
                } else {
                    return Ok(None);
                }
            }
        } else {
            Err(SIError::SilenceMissing)
        }
    }

    /// Slave feature: listen the line until request receive, then stop listening. Returns early if receive
    /// SerialMessage::SetMode(Mode::Stop) or SerialMessage::Send(). Almost SerialMessage are handled silently
    /// by self.read_message().
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    pub async fn wait_for_request(&mut self) -> Result<Option<SerialMessage>, SIError> {
        if let Some(silence) = self.silence {
            loop {
                self.status = Status::Read;
                let result = self.read_until_silence(&silence).await;
                self.status = Status::None;
                let read: Option<SerialMessage> = match result {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(e);
                    }
                };
                if let Some(msg) = read {
                    match msg {
                        SerialMessage::Send(data) => {
                            return Ok(Some(SerialMessage::Send(data.clone())));
                        }
                        SerialMessage::SetMode(mode) => {
                            if mode == Mode::Stop {
                                return Ok(Some(SerialMessage::SetMode(Mode::Stop)));
                            } else {
                                self.send_message(SerialMessage::Error(
                                    SIError::StopModeBeforeChange,
                                ))
                                    .await?;
                                continue;
                            }
                        }
                        _ => {
                            continue;
                        }
                    }
                } else {
                    return Ok(None);
                }
            }
        } else {
            Err(SIError::SilenceMissing)
        }
    }

    
    /// Master loop
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    fn run_master(&mut self) -> Result<Option<Mode>, SIError> {
        log::debug!("SerialInterface::run_master()");
        loop {
            match self.read_message() {
                Ok(msg) => {
                    if let Some(msg) = msg {
                        match msg {
                            SerialMessage::SetMode(mode) => {
                                if mode == Mode::Stop {
                                    return Ok(Some(Mode::Stop));
                                }
                            }
                            SerialMessage::Send(data) => {
                                match self.write_read(data, &self.timeout.clone()) {
                                    Ok(msg) => {
                                        if let Some(SerialMessage::SetMode(Mode::Stop)) = msg {
                                            return Ok(Some(Mode::Stop));
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("{:?}", e);
                                    }
                                }
                            }
                            _ => {
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("{:?}", e);
                }
            }
        }
    }

    /// Master loop
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    async fn run_master(&mut self) -> Result<Option<Mode>, SIError> {
        log::debug!("SerialInterface::run_master()");
        loop {
            match self.read_message().await {
                Ok(msg) => {
                    if let Some(msg) = msg {
                        match msg {
                            SerialMessage::SetMode(mode) => {
                                if mode == Mode::Stop {
                                    return Ok(Some(Mode::Stop));
                                }
                            }
                            SerialMessage::Send(data) => {
                                match self.write_read(data, &self.timeout.clone()).await {
                                    Ok(msg) => {
                                        if let Some(SerialMessage::SetMode(Mode::Stop)) = msg {
                                            return Ok(Some(Mode::Stop));
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("{:?}", e);
                                    }
                                }
                            }
                            _ => {
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("{:?}", e);
                }
            }
        }
    }

    
    
    /// Master stream loop
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    fn run_master_stream(&mut self) -> Result<Option<Mode>, SIError> {
        log::debug!("SerialInterface::run_master_stream()");
        loop {
            match self.read_message() {
                Ok(msg) => {
                    if let Some(msg) = msg {
                        match msg {
                            SerialMessage::SetMode(mode) => {
                                if mode == Mode::Stop {
                                    return Ok(Some(Mode::Stop));
                                }
                            }
                            SerialMessage::Send(data) => {
                                if let Err(e) = self.write_read_stream(data, &self.timeout.clone()) {
                                    log::error!("{:?}", e);
                                }
                            }
                            _ => {
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("{:?}", e);
                }
            }
        }
    }

    /// Master stream loop
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    async fn run_master_stream(&mut self) -> Result<Option<Mode>, SIError> {
        log::debug!("SerialInterface::run_master_stream()");
        loop {
            match self.read_message().await {
                Ok(msg) => {
                    if let Some(msg) = msg {
                        match msg {
                            SerialMessage::SetMode(mode) => {
                                if mode == Mode::Stop {
                                    return Ok(Some(Mode::Stop));
                                }
                            }
                            SerialMessage::Send(data) => {
                                if let Err(e) = self.write_read_stream(data, &self.timeout.clone()).await {
                                    log::error!("{:?}", e);
                                }
                            }
                            _ => {
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("{:?}", e);
                }
            }
        }
    }

  
    
    
    /// Slave loop
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    fn run_slave(&mut self) -> Result<Option<Mode>, SIError> {
        log::debug!("SerialInterface::run_slave()");
        loop {
            match self.wait_for_request() {
                Ok(msg) => {
                    if let Some(SerialMessage::SetMode(Mode::Stop)) = msg {
                        return Ok(Some(Mode::Stop));
                    }
                }
                Err(e) => {
                    log::error!("{:?}", e);
                }
            }
        }
    }

    /// Slave loop
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    async fn run_slave(&mut self) -> Result<Option<Mode>, SIError> {
        log::debug!("SerialInterface::run_slave()");
        loop {
            match self.wait_for_request().await {
                Ok(msg) => {
                    if let Some(SerialMessage::SetMode(Mode::Stop)) = msg {
                        return Ok(Some(Mode::Stop));
                    }
                }
                Err(e) => {
                    log::error!("{:?}", e);
                }
            }
        }
    }
    
    
    /// Sniff loop
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    fn run_sniff(&mut self) -> Result<Option<Mode>, SIError> {
        log::debug!("SerialInterface::run_sniff()");
        loop {
            match self.listen() {
                Ok(msg) => {
                    if let Some(Mode::Stop) = msg {
                        return Ok(Some(Mode::Stop));
                    }
                }
                Err(e) => {
                    log::error!("SerialInterface::run_sniff():{:?}", e.clone());
                    return Err(e);
                }
            }
        }
    }

    /// Sniff loop
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    async fn run_sniff(&mut self) -> Result<Option<Mode>, SIError> {
        log::debug!("SerialInterface::run_sniff()");
        loop {
            match self.listen().await {
                Ok(msg) => {
                    if let Some(Mode::Stop) = msg {
                        return Ok(Some(Mode::Stop));
                    }
                }
                Err(e) => {
                    log::error!("SerialInterface::run_sniff():{:?}", e.clone());
                    return Err(e);
                }
            }
        }
    }

    
    
    /// Main loop
    #[cfg(not(feature = "async-channel"))]
    #[allow(unused)]
    pub async fn start(&mut self) {
        log::debug!("SerialInterface::run()");
        loop {
            sleep(Duration::from_nanos(10)).await;
            match &self.mode {
                Mode::Stop => {
                    let result = self.read_message();
                    match result {
                        Ok(msg) => {
                            if let Some(SerialMessage::SetMode(mode)) = msg {
                                log::info!("SerialInterface::switch mode to {:?}", &mode);
                                self.mode = mode;
                            }
                        }
                        Err(e) => {
                            log::error!("Mode Stop: {:?}", e);
                        }
                    }
                }
                Mode::Master => {
                    let result = self.run_master();
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                            log::info!("SerialInterface::switch mode to Mode::Stop");
                            self.mode = Mode::Stop;
                        }
                    }
                }
                Mode::Slave => {
                    let result = self.run_slave();
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                            log::info!("SerialInterface::switch mode to Mode::Stop");
                            self.mode = Mode::Stop;
                        }
                    }
                }
                Mode::Sniff => {
                    let result = self.run_sniff();
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                            log::info!("SerialInterface::switch mode to Mode::Stop");
                            self.mode = Mode::Stop;
                        }
                    }
                }
                Mode::MasterStream => {
                    let result = self.run_master_stream();
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                            log::info!("SerialInterface::switch mode to Mode::Stop");
                            self.mode = Mode::Stop;
                        }
                    }
                }
            }
        }
    }

    /// Main loop
    #[cfg(feature = "async-channel")]
    #[allow(unused)]
    pub async fn start(&mut self) {
        log::debug!("SerialInterface::run()");
        loop {
            sleep(Duration::from_nanos(10)).await;
            match &self.mode {
                Mode::Stop => {
                    let result = self.read_message().await;
                    match result {
                        Ok(msg) => {
                            if let Some(SerialMessage::SetMode(mode)) = msg {
                                log::info!("SerialInterface::switch mode to {:?}", &mode);
                                self.mode = mode;
                            }
                        }
                        Err(e) => {
                            log::error!("Mode Stop: {:?}", e);
                        }
                    }
                }
                Mode::Master => {
                    let result = self.run_master().await;
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                            log::info!("SerialInterface::switch mode to Mode::Stop");
                            self.mode = Mode::Stop;
                        }
                    }
                }
                Mode::Slave => {
                    let result = self.run_slave().await;
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                            log::info!("SerialInterface::switch mode to Mode::Stop");
                            self.mode = Mode::Stop;
                        }
                    }
                }
                Mode::Sniff => {
                    let result = self.run_sniff().await;
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                            log::info!("SerialInterface::switch mode to Mode::Stop");
                            self.mode = Mode::Stop;
                        }
                    }
                }
                Mode::MasterStream => {
                    let result = self.run_master_stream().await;
                    match result {
                        Ok(msg) => {
                            if let Some(Mode::Stop) = msg {
                                log::info!("SerialInterface::switch mode to Mode::Stop");
                                self.mode = Mode::Stop;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                            log::info!("SerialInterface::switch mode to Mode::Stop");
                            self.mode = Mode::Stop;
                        }
                    }
                }
            }
        }
    }
}
