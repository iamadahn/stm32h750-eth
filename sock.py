import socket

server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

server_address = ('192.168.31.222', 8000)
print(f"Starting up on {server_address}")
server_socket.bind(server_address)

server_socket.listen(1)

while True:
    print("Waiting for a connection...")
    connection, client_address = server_socket.accept()

    try:
        print(f"Connection from {client_address}")

        while True:
            data = connection.recv(1024)
            if data:
                print(f"Received: {data.decode('utf-8')}")
                connection.sendall(data)
            else:
                print(f"No more data from {client_address}")
                break

    finally:
        connection.close()
