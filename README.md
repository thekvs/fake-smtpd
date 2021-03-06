# Эмулятор SMTP сервера

Данная утилита реализует упрощенный вариант эмулятора SMTP сервера.

## Установка компилятора Rust

Для установки компилятора языка программирования Rust необходимо выполнить следующую команду и затем следовать инструкциям в терминале: `curl https://sh.rustup.rs -sSf | sh`

Подробнее см. [на официальном сайте](https://www.rust-lang.org/en-US/install.html).

## Сборка

Сборка возможна несколькими способами:

1. Через вызов утилиты `make`. В этом случае будет собран deb пакет, однако для этого необходимо наличие установленной вспомогательной утилиты [fpm](https://github.com/jordansissel/fpm).
1. Непосредственно через экосистему Rust:
	1. Сборка с оптимизацией без отладочных символов: `cargo build --release`. Исполнимый файл: `target/release/fake-smtpd`
	1. Сборка без оптимизации с отладочными символами: `cargo build`. Исполнимый файл: `target/debug/fake-smtpd`

## Использование

Сервер надо запускать на стороне "жертвы". Здесь и далее предполагаем, что сервер жертвы имеет IP адрес `192.168.1.1`.

Примеры вариантов запуска:

1. `fake-smtpd --address 192.168.1.1:25 --workers 1500` -- сервер запускается на `192.168.1.1:25` в режиме **приема всех** входящих писем, Одновременно может обслуживаться не более 1500 соединений.
1. `fake-smtpd --address 192.168.1.1:25 --workers 1500 --reject-ratio 1` -- аналогично предыдущему, но теперь **все** входящие письма будут **отклоняться** с ошибкой отсутствия пользователя.
1. `fake-smtpd --address 192.168.1.1:25 --workers 1500 --reject-ratio 0.5` -- аналогично предыдущему, но теперь только 50% входящих писем будут **отклоняться** с ошибкой отсутствия пользователя.

Более подробную справку по поддерживаемым опциям можно получить, запустив программу с ключем `--help`.

## Совместное использование с утилитой **smtpflood**

1. Режим приема **всех** писем:
	1. Запускаем сервер на стороне *жертвы* (возможно, понадобится запуск от пользователя `root`): `fake-smtpd --address 192.168.1.1:25 --workers 1500`
	1. Запускаем утилиту `smtpflood` на клиентах: `smtpflood -address 192.168.1.1:25 -domain mail.ru -duration 30m -workers 500`. При этом важно следить, чтобы общее число worker'ов на клиентах не превосходило число worker'ов на сервере.
1. Режим дропа **всех** писем. В этом случае эмулируем SMTP DDoS атаку путем рассылки писем на *несуществующие* email адреса:
	1. Запускаем сервер на стороне *жертвы* (возможно, понадобится запуск от root): `fake-smtpd --address 192.168.1.1:25 --workers 1500 --reject-ratio 1`
	1. Запускаем утилиту `smtpflood` на клиентах: `smtpflood -address 192.168.1.1:25 -domain mail.ru -duration 30m -workers 500`. При этом важно следить, чтобы общее число worker'ов на клиентах не превосходило число worker'ов на сервере. Также в этом случае желательно перенаправить вывод утилиты `smtpflood` в `/dev/null`.

## Настройка ОС

**Важно!** На стороне запуска утилиты `smtpflood` **необходимо** включить опцию ядра Linux `tcp_tw_reuse`. Сделать это можно следующим образом из командной строки: `sudo sysctl -w net.ipv4.tcp_tw_reuse=1`. Для того, чтобы данная опция включалась автоматически при загрузке системы, нужно отредактировать файл `/etc/sysctl.conf` и добавить в него строку `net.ipv4.tcp_tw_reuse=1`.

Подробности об опции `tcp_tw_reuse` см. [здесь](https://stackoverflow.com/questions/8893888/dropping-of-connections-with-tcp-tw-recycle/12719362#12719362)
