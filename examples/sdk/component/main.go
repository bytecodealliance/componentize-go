package main

import (
	"fmt"
	"io"
	wasiExports "pkg/cli"
	wasiSockets "pkg/sockets"
)

const numWorkers = 16

type Component struct{}

func (c *Component) Run() error {
	socket, err := wasiSockets.NewSocket(wasiSockets.IpAddressFamilyIpv4)
	if err != nil {
		return err
	}

	if err := socket.Bind("0.0.0.0:6767"); err != nil {
		return err
	}

	listener, err := socket.Listen()
	if err != nil {
		return err
	}
	defer listener.Close()

	connCh := make(chan *wasiSockets.TcpSocket, numWorkers)

	for range numWorkers {
		go func() {
			for conn := range connCh {
				handleConn(conn)
			}
		}()
	}

	for {
		conn, err := listener.Accept()
		if err != nil {
			close(connCh)
			if err == io.EOF {
				return nil
			}
			return err
		}
		connCh <- conn
	}
}

func handleConn(conn *wasiSockets.TcpSocket) {
	body := "Hello from Go + wasi:sockets!"
	response := fmt.Sprintf(
		"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: %d\r\nConnection: close\r\n\r\n%s",
		len(body), body,
	)
	conn.Write([]byte(response))
}

func init() {
	wasiExports.RegisterExports(&Component{})
}

func main() {}
