#include <stdio.h>
#include <fcntl.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <linux/limits.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/wait.h>

void connect_process(char* label) {
    char in_path[PATH_MAX] = {0};
    sprintf(in_path, "/tmp/%s", label);
    const int input = socket(AF_UNIX, SOCK_RAW, 0);

    struct sockaddr_un server_addr;

    server_addr.sun_family = AF_UNIX;
    strncpy(server_addr.sun_path, in_path, sizeof(server_addr.sun_path));
    const int slen = sizeof(server_addr);

    if (0 != connect(input, (struct sockaddr *) &server_addr, slen)) {
        perror("Failed to connect");
        exit(-3);
    }

    while (1) {
        char buffer[1000] = {0};
        const int read_bytes = read(STDIN_FILENO, buffer, sizeof(buffer));
        if (read_bytes == 0) {
            break;
        }
        write(input, buffer, read_bytes);
    }

    exit(0);
}

void run_process(char* label, char** args) {
    char in_path[PATH_MAX] = {0};
    sprintf(in_path, "/tmp/%s", label);
    unlink(in_path);

    const int server_socket = socket(AF_UNIX, SOCK_RAW, 0);

    struct sockaddr_un server_addr;

    server_addr.sun_family = AF_UNIX;
    strncpy(server_addr.sun_path, in_path, sizeof(server_addr.sun_path));
    const int slen = sizeof(server_addr);

    if (0 != bind(server_socket, (struct sockaddr *) &server_addr, slen)) {
        perror("Failed to bind");
        exit(-3);
    }

    int input[2] = {0};
    if (0 != pipe(input)) {
        perror("Failed to create pipe");
        exit(-5);
    }

    const pid_t pid = fork();
    if (pid < 0) {
        perror("Failed to fork");
        exit(-6);
    }
    if (pid == 0) {
        close(input[1]);
        if (-1 == dup2(input[0], STDIN_FILENO)) {
            perror("Failed to dup2");
            exit(-7);
        }

        execv(args[0], args);
        perror("Execve failed");
    } else {
        close(input[0]);
        char buffer[1000] = {0};
        while (1) {
            const int num_bytes = read(server_socket, buffer, sizeof(buffer));
            int total_sent_bytes = 0;
            while (total_sent_bytes < num_bytes) {
                const int sent_bytes = write(input[1], buffer+total_sent_bytes, num_bytes-total_sent_bytes);
                if (sent_bytes == 0) {
                    kill(pid, SIGKILL);
                    waitpid(pid, NULL, 0);
                    exit(0);
                }
                total_sent_bytes += sent_bytes;
            }
        }
    }
}

int main(const int argc, char** argv)
{
    char* label = argv[1];

    if (argc < 2) {
        exit(-1);
    }
    if (argc == 2) {
        connect_process(label);
    }
    if (argc >= 3) {
        run_process(label, &argv[2]);
    }
}