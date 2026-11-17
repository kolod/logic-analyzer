// SPDX-License-Identifier: GPL-3.0-only
//
// la_rig: a pattern generator for testing a logic analyzer (Arduino Nano, ATmega328P).
//
// Pins D2..D9 are outputs. The 8-bit value v of a pattern is written so that analyzer input i is
// bit i of v:   PORTD bits 2..7 = v bits 0..5   (pins D2..D7)
//               PORTB bits 0..1 = v bits 6..7   (pins D8, D9)
//
// Serial protocol, 115200 baud 8N1, ASCII lines ending in '\n'. Every line is answered with
// one line. After a reset the sketch prints "LA-RIG 1" once.
//
//   HELLO              -> LA-RIG 1
//   STOP               -> OK     all pins low, pattern stopped
//   HOLD <hex>         -> OK     two hex digits held on the pins
//   COUNTER <step_us>  -> OK     8-bit up counter, changes every step_us
//   WALK <step_us>     -> OK     one-hot walk 01, 02, 04 ... 80, 01 ..., changes every step_us
//   SQUARE <step_us>   -> OK     all eight pins toggle together every step_us
//   anything else      -> ERR    (also a step outside 20..1000000)
//
// The serial port is polled between steps, so a command takes effect within one
// step. A step of 100 us or less runs with interrupts off to keep its length steady.

#include <Arduino.h>

static const uint32_t STEP_MIN_US = 20;
static const uint32_t STEP_MAX_US = 1000000;
static const uint8_t LINE_MAX = 24;

enum Mode : uint8_t { MODE_IDLE, MODE_HOLD, MODE_COUNTER, MODE_WALK, MODE_SQUARE };

static Mode mode = MODE_IDLE;
static uint8_t value = 0;
static uint32_t step_us = 1000;

static char line[LINE_MAX];
static uint8_t line_length = 0;
static bool line_overflow = false;

// Writes v to D2..D9 with two port writes.
static inline void write_pins(uint8_t v) {
  // No interrupt may come between the two writes, or the pins would show a mixed value for as
  // long as the interrupt takes.
  uint8_t sreg = SREG;
  cli();
  PORTD = (uint8_t)((PORTD & 0x03) | ((v & 0x3F) << 2));
  PORTB = (uint8_t)((PORTB & 0xFC) | ((v >> 6) & 0x03));
  SREG = sreg;
}

static void wait_us(uint32_t us) {
  // A byte of the serial port takes 87 us and the receiver holds two: interrupts may be off for
  // at most about 100 us at a time, or a command is lost.
  if (us <= 100) {
    noInterrupts();
    delayMicroseconds((unsigned int)us);
    interrupts();
    return;
  }
  while (us > 1000) {
    delayMicroseconds(1000);
    us -= 1000;
  }
  delayMicroseconds((unsigned int)us);
}

static int hex_digit(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}

// Parses a decimal step in 20..1000000 from s; false when it is not one.
static bool parse_step(const char* s, uint32_t* out) {
  if (*s == '\0') return false;
  uint32_t n = 0;
  for (; *s != '\0'; ++s) {
    if (*s < '0' || *s > '9') return false;
    n = n * 10 + (uint32_t)(*s - '0');
    if (n > STEP_MAX_US) return false;
  }
  if (n < STEP_MIN_US) return false;
  *out = n;
  return true;
}

static void start_pattern(Mode m, uint8_t first) {
  mode = m;
  value = first;
  write_pins(value);
}

static void handle_line(char* text) {
  if (strcmp(text, "HELLO") == 0) {
    Serial.println(F("LA-RIG 1"));
  } else if (strcmp(text, "STOP") == 0) {
    mode = MODE_IDLE;
    write_pins(0);
    Serial.println(F("OK"));
  } else if (strncmp(text, "HOLD ", 5) == 0 && strlen(text) == 7 && hex_digit(text[5]) >= 0 &&
             hex_digit(text[6]) >= 0) {
    start_pattern(MODE_HOLD, (uint8_t)(hex_digit(text[5]) * 16 + hex_digit(text[6])));
    Serial.println(F("OK"));
  } else if (strncmp(text, "COUNTER ", 8) == 0 && parse_step(text + 8, &step_us)) {
    start_pattern(MODE_COUNTER, 0);
    Serial.println(F("OK"));
  } else if (strncmp(text, "WALK ", 5) == 0 && parse_step(text + 5, &step_us)) {
    start_pattern(MODE_WALK, 0x01);
    Serial.println(F("OK"));
  } else if (strncmp(text, "SQUARE ", 7) == 0 && parse_step(text + 7, &step_us)) {
    start_pattern(MODE_SQUARE, 0xFF);
    Serial.println(F("OK"));
  } else {
    Serial.println(F("ERR"));
  }
}

// Reads the received bytes without waiting. (The Arduino serial library owns the receive
// interrupt, so the bytes must be taken from its buffer, not straight from the register.)
static void poll_serial() {
  while (Serial.available() > 0) {
    char c = (char)Serial.read();
    if (c == '\r') continue;
    if (c == '\n') {
      if (line_overflow) {
        Serial.println(F("ERR"));
      } else {
        line[line_length] = '\0';
        handle_line(line);
      }
      line_length = 0;
      line_overflow = false;
    } else if (line_length < LINE_MAX - 1) {
      line[line_length++] = c;
    } else {
      line_overflow = true;
    }
  }
}

void setup() {
  DDRD |= 0xFC;  // D2..D7
  DDRB |= 0x03;  // D8, D9
  write_pins(0);
  Serial.begin(115200);
  Serial.println(F("LA-RIG 1"));
}

void loop() {
  poll_serial();
  switch (mode) {
    case MODE_IDLE:
    case MODE_HOLD:
      return;
    case MODE_COUNTER:
      wait_us(step_us);
      value++;
      break;
    case MODE_WALK:
      wait_us(step_us);
      value = (uint8_t)((value << 1) | (value >> 7));
      break;
    case MODE_SQUARE:
      wait_us(step_us);
      value = (uint8_t)~value;
      break;
  }
  write_pins(value);
}
