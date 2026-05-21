package sockets

import (
	"encoding/binary"
	"fmt"
	"io"
	"net/netip"
	wasiSockets "pkg/bindings/imports/wasi_sockets_types"
	"strconv"
	"time"

	witTypes "go.bytecodealliance.org/pkg/wit/types"
)

type IpAddressFamily uint8

const (
	_ IpAddressFamily = iota
	IpAddressFamilyIpv4
	IpAddressFamilyIpv6
)

type TcpSocket struct {
	inner wasiSockets.TcpSocket
	rx    *witTypes.StreamReader[uint8]
	tx    *witTypes.StreamWriter[uint8]
}

// Create a new TCP socket.
func NewSocket(af IpAddressFamily) (TcpSocket, error) {
	result := wasiSockets.TcpSocketCreate(toWasiIpAddressFamily(af))
	if result.IsErr() {
		return TcpSocket{}, fmt.Errorf("Error creating TCP socket: %w", fromWitErrorCode(result.Err()))
	}

	return TcpSocket{
		inner: *result.Ok(),
	}, nil
}

// Bind the socket to the provided IP address and port
func (s *TcpSocket) Bind(address string) error {
	socketAddr, err := toWasiIpSockAddr(address)
	if err != nil {
		return err
	}
	result := s.inner.Bind(socketAddr)
	if result.IsErr() {
		return fmt.Errorf("failed to bind to socket: %w", fromWitErrorCode(result.Err()))
	}

	return nil
}

// Connect to a remote endpoint.
func (s *TcpSocket) Connect(address string) error {
	socketAddr, err := toWasiIpSockAddr(address)
	if err != nil {
		return err
	}
	result := s.inner.Connect(socketAddr)
	if result.IsErr() {
		return fmt.Errorf("failed to connect to socket: %w", fromWitErrorCode(result.Err()))
	}
	return nil
}

type Listener struct {
	inner *witTypes.StreamReader[*wasiSockets.TcpSocket]
}

func (l *Listener) Accept() (*TcpSocket, error) {
	if l.inner.WriterDropped() {
		return nil, io.EOF
	}
	buf := make([]*wasiSockets.TcpSocket, 1)
	count := l.inner.Read(buf)
	if count == 0 {
		return nil, io.EOF
	}

	sock := &TcpSocket{inner: *buf[0]}

	rx, _ := sock.inner.Receive()
	sock.rx = rx

	tx, txReader := wasiSockets.MakeStreamU8()
	sock.inner.Send(txReader)
	sock.tx = tx

	return sock, nil
}

func (l *Listener) Close() error {
	l.inner.Drop()
	return nil
}

// Start listening and return a stream of new inbound connections.
func (s *TcpSocket) Listen() (*Listener, error) {
	result := s.inner.Listen()
	if result.IsErr() {
		return nil, fmt.Errorf("failed to listen to socket: %w", fromWitErrorCode(result.Err()))
	}

	return &Listener{
		inner: result.Ok(),
	}, nil
}

// Write data to TCP stream
func (s *TcpSocket) Write(b []byte) (int, error) {
	s.tx.WriteAll(b)
	return len(b), nil
}

// Read data from TCP stream
func (s *TcpSocket) Read(b []byte) (int, error) {
	if s.rx.WriterDropped() {
		return 0, io.EOF
	}
	n := s.rx.Read(b)
	return int(n), nil
}

// Get the bound local address.
func (s *TcpSocket) GetLocalAddress() (netip.AddrPort, error) {
	result := s.inner.GetLocalAddress()
	if result.IsErr() {
		return netip.AddrPort{}, fmt.Errorf("failed to get local address%w", fromWitErrorCode(result.Err()))
	}

	return fromWasiIpSocketAddr(result.Ok()), nil
}

// Get the remote address.
func (s *TcpSocket) GetRemoteAddress() (netip.AddrPort, error) {
	result := s.inner.GetRemoteAddress()
	if result.IsErr() {
		return netip.AddrPort{}, fmt.Errorf("failed to get local address%w", fromWitErrorCode(result.Err()))
	}

	return fromWasiIpSocketAddr(result.Ok()), nil
}

// Whether this is a IPv4 or IPv6 socket.
func (s *TcpSocket) GetAddressFamily() IpAddressFamily {
	switch s.inner.GetAddressFamily() {
	case wasiSockets.IpAddressFamilyIpv4:
		return IpAddressFamilyIpv4
	case wasiSockets.IpAddressFamilyIpv6:
		return IpAddressFamilyIpv6
	default:
		panic("GetAddressFamily has retrieved a 3rd, heretofore unknown IpAddressFamily type")
	}
}

// Hints the desired listen queue size. Host implementations might ignore this.
func (s *TcpSocket) SetListenBacklogSize(size uint64) error {
	err := s.inner.SetListenBacklogSize(size)
	if err.IsErr() {
		return fromWitErrorCode(err.Err())
	}
	return nil
}

// Indicates whether keepalive is enabled or disabled.
func (s *TcpSocket) GetKeepAliveEnabled() (bool, error) {
	result := s.inner.GetKeepAliveEnabled()
	if result.IsErr() {
		return false, fromWitErrorCode(result.Err())
	}
	return result.Ok(), nil
}

// Enables or disables keepalive.
func (s *TcpSocket) SetKeepAliveEnabled(v bool) error {
	result := s.inner.SetKeepAliveEnabled(v)
	if result.IsErr() {
		return fromWitErrorCode(result.Err())
	}
	return nil
}

// Amount of time the connection has been set to be idle before TCP starts
// sending keepalive packets.
func (s *TcpSocket) GetKeepAliveIdleTime() (time.Duration, error) {
	result := s.inner.GetKeepAliveIdleTime()
	if result.IsErr() {
		return time.Duration(-1), fromWitErrorCode(result.Err())
	}
	return time.Duration(result.Ok()), nil
}

// Amount of time the connection has to be idle before TCP starts
// sending keepalive packets.
func (s *TcpSocket) SetKeepAliveIdleTime(duration time.Duration) error {
	if duration < 0 {
		return fmt.Errorf("duration must be >= 0")
	}
	err := s.inner.SetKeepAliveIdleTime(uint64(duration))
	if err.IsErr() {
		return fromWitErrorCode(err.Err())
	}
	return nil
}

// The time between keepalive packets.
func (s *TcpSocket) GetKeepAliveInterval() (time.Duration, error) {
	result := s.inner.GetKeepAliveInterval()
	if result.IsErr() {
		return time.Duration(-1), fromWitErrorCode(result.Err())
	}
	return time.Duration(result.Ok()), nil
}

// The time between keepalive packets.
func (s *TcpSocket) SetKeepAliveInterval(duration time.Duration) error {
	if duration < 0 {
		return fmt.Errorf("duration must be >= 0")
	}
	err := s.inner.SetKeepAliveInterval(uint64(duration))
	if err.IsErr() {
		return fromWitErrorCode(err.Err())
	}
	return nil
}

// The maximum amount of keepalive packets TCP should send before
// aborting the connection.
func (s *TcpSocket) GetKeepAliveCount() (uint32, error) {
	result := s.inner.GetKeepAliveCount()
	if result.IsErr() {
		return 0, fromWitErrorCode(result.Err())
	}
	return result.Ok(), nil
}

// The maximum amount of keepalive packets TCP should send before
// aborting the connection.
func (s *TcpSocket) SetKeepAliveCount(v uint32) error {
	err := s.inner.SetKeepAliveCount(v)
	if err.IsErr() {
		return fromWitErrorCode(err.Err())
	}
	return nil
}

// Equivalent to the IP_TTL & IPV6_UNICAST_HOPS socket options.
func (s *TcpSocket) GetHopLimit() (uint8, error) {
	result := s.inner.GetHopLimit()
	if result.IsErr() {
		return 0, fromWitErrorCode(result.Err())
	}
	return result.Ok(), nil
}

// Equivalent to the IP_TTL & IPV6_UNICAST_HOPS socket options.
func (s *TcpSocket) SetHopLimit(v uint8) error {
	err := s.inner.SetHopLimit(v)
	if err.IsErr() {
		return fromWitErrorCode(err.Err())
	}
	return nil
}

// Kernel buffer space reserved for receiving on this socket.
func (s *TcpSocket) GetReceiveBufferSize() (uint64, error) {
	result := s.inner.GetReceiveBufferSize()
	if result.IsErr() {
		return 0, fromWitErrorCode(result.Err())
	}
	return result.Ok(), nil
}

// Kernel buffer space reserved for receiving on this socket.
func (s *TcpSocket) SetReceiveBufferSize(size uint64) error {
	err := s.inner.SetReceiveBufferSize(size)
	if err.IsErr() {
		return fromWitErrorCode(err.Err())
	}
	return nil
}

// Kernel buffer space reserved for sending on this socket.
func (s *TcpSocket) GetSendBufferSize() (uint64, error) {
	result := s.inner.GetSendBufferSize()
	if result.IsErr() {
		return 0, fromWitErrorCode(result.Err())
	}
	return result.Ok(), nil
}

// Kernel buffer space reserved for sending on this socket.
func (s *TcpSocket) SetSendBufferSize(size uint64) error {
	err := s.inner.SetSendBufferSize(size)
	if err.IsErr() {
		return fromWitErrorCode(err.Err())
	}
	return nil
}

func toWasiIpSockAddr(addr string) (wasiSockets.IpSocketAddress, error) {
	ip, err := netip.ParseAddrPort(addr)
	if err != nil {
		return wasiSockets.IpSocketAddress{}, err
	}

	var socketAddr wasiSockets.IpSocketAddress

	if ip.Addr().Is4() {
		b := ip.Addr().As4()
		socketAddr = wasiSockets.MakeIpSocketAddressIpv4(wasiSockets.Ipv4SocketAddress{
			Address: witTypes.Tuple4[uint8, uint8, uint8, uint8]{
				F0: b[0],
				F1: b[1],
				F2: b[2],
				F3: b[3],
			},
			Port: ip.Port(),
		})
	} else if ip.Addr().Is6() {
		b := ip.Addr().As16()

		// Zone == ScopeId
		var scope uint32
		if z := ip.Addr().Zone(); z != "" {
			n, err := strconv.ParseUint(z, 10, 32)
			if err != nil {
				// Non-numeric zone (e.g. "eth0")
				return wasiSockets.IpSocketAddress{}, fmt.Errorf("non-numeric zone %q: %w", z, err)
			}
			scope = uint32(n)
		}
		socketAddr = wasiSockets.MakeIpSocketAddressIpv6(wasiSockets.Ipv6SocketAddress{
			FlowInfo: 0, // Setting to 0 since the net/netip package doesn't expose this
			Address: witTypes.Tuple8[uint16, uint16, uint16, uint16, uint16, uint16, uint16, uint16]{
				F0: binary.BigEndian.Uint16(b[0:2]),
				F1: binary.BigEndian.Uint16(b[2:4]),
				F2: binary.BigEndian.Uint16(b[4:6]),
				F3: binary.BigEndian.Uint16(b[6:8]),
				F4: binary.BigEndian.Uint16(b[8:10]),
				F5: binary.BigEndian.Uint16(b[10:12]),
				F6: binary.BigEndian.Uint16(b[12:14]),
				F7: binary.BigEndian.Uint16(b[14:16]),
			},
			Port:    ip.Port(),
			ScopeId: scope,
		})

	} else {
		panic("parsed ip addr is neither ipv4 nor ipv6")
	}

	return socketAddr, nil
}

func fromWasiIpSocketAddr(addr wasiSockets.IpSocketAddress) netip.AddrPort {
	switch addr.Tag() {
	case wasiSockets.IpSocketAddressIpv4:
		v4 := addr.Ipv4()
		b := v4.Address
		ip := netip.AddrFrom4([4]byte{b.F0, b.F1, b.F2, b.F3})
		return netip.AddrPortFrom(ip, v4.Port)
	case wasiSockets.IpSocketAddressIpv6:
		v6 := addr.Ipv6()
		a := v6.Address
		var b [16]byte
		binary.BigEndian.PutUint16(b[0:2], a.F0)
		binary.BigEndian.PutUint16(b[2:4], a.F1)
		binary.BigEndian.PutUint16(b[4:6], a.F2)
		binary.BigEndian.PutUint16(b[6:8], a.F3)
		binary.BigEndian.PutUint16(b[8:10], a.F4)
		binary.BigEndian.PutUint16(b[10:12], a.F5)
		binary.BigEndian.PutUint16(b[12:14], a.F6)
		binary.BigEndian.PutUint16(b[14:16], a.F7)
		ip := netip.AddrFrom16(b)
		return netip.AddrPortFrom(ip, v6.Port)
	default:
		panic(fmt.Sprintf("unimplemented IpSocketAddress type: %v", addr))
	}
}

func toWasiIpAddressFamily(af IpAddressFamily) wasiSockets.IpAddressFamily {
	switch af {
	case IpAddressFamilyIpv4:
		return wasiSockets.IpAddressFamilyIpv4
	case IpAddressFamilyIpv6:
		return wasiSockets.IpAddressFamilyIpv6
	default:
		panic("Wow, who could've guessed that this code would live to see a THIRD IpAddress family?!")
	}
}

func fromWitErrorCode(err wasiSockets.ErrorCode) error {
	switch err.Tag() {
	case wasiSockets.ErrorCodeOther:
		return fmt.Errorf("other error")
	case wasiSockets.ErrorCodeAccessDenied:
		return fmt.Errorf("access denied")
	case wasiSockets.ErrorCodeNotSupported:
		return fmt.Errorf("operation is not supported")
	case wasiSockets.ErrorCodeInvalidArgument:
		return fmt.Errorf("one of the argumenets is invalid")
	case wasiSockets.ErrorCodeOutOfMemory:
		return fmt.Errorf("out of memory")
	case wasiSockets.ErrorCodeTimeout:
		return fmt.Errorf("not enough memory to complete the operation")
	case wasiSockets.ErrorCodeInvalidState:
		return fmt.Errorf("operation is not valid in the socket's current state")
	case wasiSockets.ErrorCodeAddressNotBindable:
		return fmt.Errorf("bind operation failed because the provided address is not an address that the `network` can bind to")
	case wasiSockets.ErrorCodeAddressInUse:
		return fmt.Errorf("bind operation failed because the provided address is already in use or because there are no ephemeral ports available")
	case wasiSockets.ErrorCodeRemoteUnreachable:
		return fmt.Errorf("remote address is not reachable")
	case wasiSockets.ErrorCodeConnectionRefused:
		return fmt.Errorf("TCP connection was forcefully rejected")
	case wasiSockets.ErrorCodeConnectionReset:
		return fmt.Errorf("TCP connection was reset")
	case wasiSockets.ErrorCodeConnectionAborted:
		return fmt.Errorf("TCP connection was aborted")
	case wasiSockets.ErrorCodeDatagramTooLarge:
		return fmt.Errorf("size of a datagram sent to a UDP socket exceeded the maximum supported size")
	default:
		panic("unimplemented error code")
	}
}
