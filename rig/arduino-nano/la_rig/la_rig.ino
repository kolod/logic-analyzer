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
//   LCDP <4|8> <cols> <rows> <step_us>
//                      -> OK     HD44780 on a parallel bus (see "HD44780" below)
//   LCDI <addr> <cols> <rows> <step_us>
//                      -> OK     HD44780 behind a PCF8574 on I2C, <addr> is 2 hex digits
//   anything else      -> ERR    (also a step outside 20..1000000)
//
// The serial port is polled between steps, so a command takes effect within one
// step. A step of 100 us or less runs with interrupts off to keep its length steady.
//
// HD44780 modes. Every second the sketch sends one whole script: the initialisation by
// instruction, then every instruction of the controller (clear, home, entry mode, display
// control, shifts, function set, CGRAM and DDRAM addresses), text and custom characters written
// to the display, and reads of the busy flag / address and of data. <cols> is 8..40, <rows> is
// 1, 2 or 4 (4 rows only up to 20 columns). The script is cut short by the next serial command.
//
//   Parallel (LCDP). <step_us> is the length of each of setup, E high and hold of a cycle.
//     v bit:   0    1    2   3    4    5    6    7
//     4-bit:   RS   RW   E   D4   D5   D6   D7   (unused, low)
//     8-bit:   RS   RW   E   D0   D1   D2   D3   D4      and D5, D6, D7 on Nano D10, D11, D12
//     (an 8-bit bus has ten lines: the analyzer sees eight, the last three leave on D10..D12)
//
//   I2C (LCDI). SCL on analyzer input 0 (D2), SDA on input 1 (D3). The PCF8574 backpack bits are
//     P0 RS, P1 RW, P2 E, P3 backlight (on), P4..P7 D4..D7. One transaction carries one byte of
//     the display: the high nibble with E high and low, then the low nibble. A read is three
//     transactions per nibble (E high, read, E low). One bit of I2C takes four steps; the
//     address and every byte are acknowledged. The bus is 4-bit only.

#include <Arduino.h>

static const uint32_t STEP_MIN_US = 20;
static const uint32_t STEP_MAX_US = 1000000;
static const uint8_t LINE_MAX = 24;

enum Mode : uint8_t { MODE_IDLE, MODE_HOLD, MODE_COUNTER, MODE_WALK, MODE_SQUARE, MODE_LCD };

enum LcdBus : uint8_t { LCD_PAR4, LCD_PAR8, LCD_I2C };

static Mode mode = MODE_IDLE;
static uint8_t value = 0;
static uint32_t step_us = 1000;
static uint8_t extra = 0;    // Nano D10..D12 (PORTB bits 2..4), bits 0..2 of this value
static uint8_t session = 0;  // changes with every command that starts or stops a pattern

static void poll_serial();

static char line[LINE_MAX];
static uint8_t line_length = 0;
static bool line_overflow = false;

// Writes v to D2..D9 with two port writes.
static inline void write_pins(uint8_t v) {
  // Both port values are worked out first and written one right after the other with interrupts
  // off: the two ports cannot change at the very same instant, but they change one AVR cycle
  // (62 ns) apart, not a microsecond, and no interrupt can come between the writes.
  uint8_t port_d = (uint8_t)((PORTD & 0x03) | ((v & 0x3F) << 2));
  uint8_t port_b = (uint8_t)((PORTB & 0xE0) | ((extra & 0x07) << 2) | ((v >> 6) & 0x03));
  uint8_t sreg = SREG;
  cli();
  PORTD = port_d;
  PORTB = port_b;
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
  session++;
  mode = m;
  value = first;
  extra = 0;
  write_pins(value);
}

// ---------------------------------------------------------------------------------------------
// HD44780
// ---------------------------------------------------------------------------------------------

static LcdBus lcd_bus = LCD_PAR4;
static uint8_t lcd_cols = 16;
static uint8_t lcd_rows = 2;
static uint8_t lcd_addr = 0x27;
static uint8_t script_session = 0;

static const uint8_t PCF_RS = 0x01, PCF_RW = 0x02, PCF_E = 0x04, PCF_BL = 0x08;

// True while the script that started with this session is still wanted. Reads the serial port,
// so a new command ends the script within one byte of the display.
static bool lcd_alive() {
  poll_serial();
  return mode == MODE_LCD && session == script_session;
}

// ----- I2C: SCL is bit 0, SDA is bit 1 of the pins; a bit takes four steps -----

static bool i2c_sda = true;  // SDA as last driven

static void i2c_line(bool scl, bool sda, uint8_t steps) {
  i2c_sda = sda;
  write_pins((uint8_t)((scl ? 1 : 0) | (sda ? 2 : 0)));
  wait_us(step_us * steps);
}

static void i2c_start() {
  i2c_line(true, true, 2);
  i2c_line(true, false, 2);   // SDA falls while SCL is high
  i2c_line(false, false, 2);
}

static void i2c_stop() {
  i2c_line(false, false, 2);
  i2c_line(true, false, 2);
  i2c_line(true, true, 2);    // SDA rises while SCL is high
}

// SCL falls first and SDA changes one step later, in the middle of the low phase: two inputs of
// an analyzer are never sampled with a skew across a simultaneous edge, which a decoder would
// read as a START or STOP.
static void i2c_bit(bool b) {
  i2c_line(false, i2c_sda, 1);
  i2c_line(false, b, 1);
  i2c_line(true, b, 2);
}

// A byte followed by the acknowledge of the receiver (SDA low).
static void i2c_byte(uint8_t b) {
  for (int8_t i = 7; i >= 0; --i) i2c_bit((b >> i) & 1);
  i2c_bit(false);
}

// A write transaction to the PCF8574.
static void i2c_write(const uint8_t* d, uint8_t n) {
  i2c_start();
  i2c_byte((uint8_t)(lcd_addr << 1));
  for (uint8_t i = 0; i < n; ++i) i2c_byte(d[i]);
  i2c_stop();
}

// A read transaction: the PCF8574 sends b, the master ends it with NACK.
static void i2c_read(uint8_t b) {
  i2c_start();
  i2c_byte((uint8_t)((lcd_addr << 1) | 1));
  for (int8_t i = 7; i >= 0; --i) i2c_bit((b >> i) & 1);
  i2c_bit(true);
  i2c_stop();
}

// ----- parallel -----

static void par_drive(bool rs, bool rw, bool e, uint8_t d) {
  uint8_t v = (uint8_t)((rs ? 1 : 0) | (rw ? 2 : 0) | (e ? 4 : 0));
  if (lcd_bus == LCD_PAR8) {
    v |= (uint8_t)((d & 0x1F) << 3);
    extra = (uint8_t)((d >> 5) & 0x07);
  } else {
    v |= (uint8_t)((d & 0x0F) << 3);
  }
  write_pins(v);
}

// One cycle of E: setup, E high, hold. d is a nibble (4-bit bus) or a byte (8-bit bus).
static void par_cycle(bool rs, bool rw, uint8_t d) {
  par_drive(rs, rw, false, d);
  wait_us(step_us);
  par_drive(rs, rw, true, d);
  wait_us(step_us);
  par_drive(rs, rw, false, d);
  wait_us(step_us);
}

// ----- the display, whatever the bus -----

// A lone nibble: the wake-up of the initialisation, sent while the display is still 8-bit.
static void lcd_nibble(uint8_t n) {
  if (lcd_bus == LCD_I2C) {
    uint8_t b = (uint8_t)((n << 4) | PCF_BL);
    uint8_t d[2] = {(uint8_t)(b | PCF_E), b};
    i2c_write(d, 2);
  } else if (lcd_bus == LCD_PAR8) {
    par_cycle(false, false, (uint8_t)(n << 4));
  } else {
    par_cycle(false, false, n);
  }
}

// A byte written to the instruction register (rs low) or the data register (rs high).
static void lcd_write(bool rs, uint8_t b) {
  if (lcd_bus == LCD_I2C) {
    uint8_t hi = (uint8_t)((b & 0xF0) | PCF_BL | (rs ? PCF_RS : 0));
    uint8_t lo = (uint8_t)(((b << 4) & 0xF0) | PCF_BL | (rs ? PCF_RS : 0));
    uint8_t d[4] = {(uint8_t)(hi | PCF_E), hi, (uint8_t)(lo | PCF_E), lo};
    i2c_write(d, 4);
  } else if (lcd_bus == LCD_PAR8) {
    par_cycle(rs, false, b);
  } else {
    par_cycle(rs, false, (uint8_t)(b >> 4));
    par_cycle(rs, false, (uint8_t)(b & 0x0F));
  }
}

// A byte read from the display (rs low: busy flag and address, rs high: data); the display
// answers with b.
static void lcd_read(bool rs, uint8_t b) {
  if (lcd_bus == LCD_I2C) {
    for (uint8_t half = 0; half < 2; ++half) {
      uint8_t nib = half == 0 ? (uint8_t)(b >> 4) : (uint8_t)(b & 0x0F);
      uint8_t ctl = (uint8_t)(PCF_BL | PCF_RW | (rs ? PCF_RS : 0));
      uint8_t up[1] = {(uint8_t)(0xF0 | ctl | PCF_E)};
      uint8_t down[1] = {(uint8_t)(0xF0 | ctl)};
      i2c_write(up, 1);
      i2c_read((uint8_t)((nib << 4) | ctl | PCF_E));
      i2c_write(down, 1);
    }
  } else if (lcd_bus == LCD_PAR8) {
    par_cycle(rs, true, b);
  } else {
    par_cycle(rs, true, (uint8_t)(b >> 4));
    par_cycle(rs, true, (uint8_t)(b & 0x0F));
  }
}

static void lcd_idle() {
  if (lcd_bus == LCD_I2C) {
    i2c_line(true, true, 1);
  } else {
    par_drive(false, false, false, 0);
  }
}

// The instruction time of the controller; the bus stays quiet that long.
static void lcd_cmd(uint8_t c) {
  if (!lcd_alive()) return;
  lcd_write(false, c);
  lcd_idle();
  wait_us(c < 4 ? 1600 : 40);
}

static void lcd_data(uint8_t d) {
  if (!lcd_alive()) return;
  lcd_write(true, d);
  lcd_idle();
  wait_us(40);
}

static void lcd_text(const char* s, uint8_t max) {
  for (uint8_t i = 0; s[i] != '\0' && i < max; ++i) lcd_data((uint8_t)s[i]);
}

// DDRAM address of the start of a row.
static uint8_t row_base(uint8_t r) {
  if (lcd_rows == 1) return 0;
  if (lcd_rows == 2) return r ? 0x40 : 0x00;
  return (uint8_t)((r & 1 ? 0x40 : 0x00) + (r & 2 ? lcd_cols : 0));
}

static void lcd_goto(uint8_t r, uint8_t c) { lcd_cmd((uint8_t)(0x80 | (row_base(r) + c))); }

static void lcd_line(uint8_t r, const char* s) {
  lcd_goto(r, 0);
  lcd_text(s, lcd_cols);
}

// 5x8 glyphs of the two custom characters.
static const uint8_t GLYPH_HEART[8] = {0x00, 0x0A, 0x1F, 0x1F, 0x0E, 0x04, 0x00, 0x00};
static const uint8_t GLYPH_ARROW[8] = {0x04, 0x08, 0x1F, 0x08, 0x04, 0x00, 0x00, 0x00};

static const char TITLE[] = "HD44780 test";

// The initialisation by instruction (the datasheet's figures for the 8-bit and 4-bit bus).
static void lcd_init() {
  uint8_t function = (uint8_t)((lcd_bus == LCD_PAR8 ? 0x30 : 0x20) | (lcd_rows > 1 ? 0x08 : 0x00));
  lcd_idle();
  wait_us(20000);
  lcd_nibble(0x3);
  wait_us(4100);
  lcd_nibble(0x3);
  wait_us(100);
  lcd_nibble(0x3);
  wait_us(100);
  if (lcd_bus != LCD_PAR8) {
    lcd_nibble(0x2);  // 4-bit from here on
    wait_us(100);
  }
  lcd_cmd(function);
  lcd_cmd(0x08);  // display off
  lcd_cmd(0x01);  // clear
  lcd_cmd(0x06);  // entry mode: increment, no shift
  lcd_cmd(0x0C);  // display on, cursor off, blink off
}

// Every instruction of the controller once, with text and reads between them.
static void lcd_demo() {
  uint8_t function = (uint8_t)((lcd_bus == LCD_PAR8 ? 0x30 : 0x20) | (lcd_rows > 1 ? 0x08 : 0x00));
  uint8_t mid = (uint8_t)(lcd_cols / 2);

  // Text on every row, clear display, return home.
  lcd_cmd(0x01);
  lcd_line(0, TITLE);
  if (lcd_rows > 1) lcd_line(1, "0123456789ABCDEFGHIJ");
  if (lcd_rows > 2) lcd_line(2, "abcdefghijklmnopqrst");
  if (lcd_rows > 2) lcd_line(3, "!\"#$%&'()*+,-./:;<=");
  lcd_cmd(0x02);  // return home

  // Busy flag and address counter: busy once, then ready; address 0 after "home".
  lcd_read(false, 0x80);
  lcd_read(false, 0x00);

  // Entry mode set: all four combinations.
  for (uint8_t em = 0x04; em <= 0x07; ++em) {
    lcd_cmd(0x01);
    lcd_goto(0, mid);
    lcd_cmd(em);
    lcd_text("ab", 2);
  }
  lcd_cmd(0x06);

  // Display on/off control: all eight combinations of D, C, B.
  for (uint8_t dc = 0x08; dc <= 0x0F; ++dc) lcd_cmd(dc);

  // Cursor or display shift: cursor left/right, display left/right.
  lcd_cmd(0x01);
  lcd_line(0, TITLE);
  lcd_cmd(0x0F);
  lcd_cmd(0x14);
  lcd_cmd(0x14);
  lcd_cmd(0x10);
  lcd_cmd(0x18);
  lcd_cmd(0x18);
  lcd_cmd(0x1C);
  lcd_cmd(0x1C);
  lcd_cmd(0x02);  // home also undoes the display shift
  lcd_cmd(0x0C);

  // Function set: 5x10 font on a one-row display, one row on a two-row display.
  if (lcd_rows == 1) {
    lcd_cmd((uint8_t)(function | 0x04));
  } else {
    lcd_cmd((uint8_t)(function & ~0x08));
  }
  lcd_cmd(function);

  // CGRAM: two custom characters, shown with the codes 0 and 1.
  lcd_cmd(0x40);
  for (uint8_t i = 0; i < 8; ++i) lcd_data(GLYPH_HEART[i]);
  for (uint8_t i = 0; i < 8; ++i) lcd_data(GLYPH_ARROW[i]);
  lcd_cmd(0x01);
  lcd_goto(0, 0);
  lcd_data(0x00);
  lcd_data(0x01);
  lcd_text(" custom", (uint8_t)(lcd_cols - 2));

  // Characters from the upper half of the font table.
  lcd_goto(lcd_rows > 1 ? 1 : 0, lcd_rows > 1 ? 0 : 9);
  for (uint8_t c = 0xA6; c <= 0xAD; ++c) lcd_data(c);

  // Reads: DDRAM data at address 3, CGRAM data at address 2.
  lcd_cmd(0x83);
  lcd_read(false, 0x03);   // address counter after the set address
  lcd_read(true, '4');     // TITLE[3]; the counter moves on
  lcd_cmd(0x42);
  lcd_read(true, GLYPH_HEART[2]);

  // Set DDRAM address on every row, and a few bytes there.
  for (uint8_t r = 0; r < lcd_rows; ++r) {
    lcd_goto(r, 0);
    lcd_data((uint8_t)('1' + r));
  }

  // Clear display at the end, and the display is left on, cursor off.
  lcd_cmd(0x01);
  lcd_text("done", lcd_cols);
}

static void lcd_script() {
  script_session = session;
  lcd_init();
  lcd_demo();
  lcd_idle();
}

static void start_lcd(LcdBus bus, uint8_t cols, uint8_t rows) {
  session++;
  mode = MODE_LCD;
  lcd_bus = bus;
  lcd_cols = cols;
  lcd_rows = rows;
  extra = 0;
  write_pins(bus == LCD_I2C ? 0x03 : 0x00);
}

// Takes one unsigned number from *s, up to a space or the end; false when it has no digits or a
// character that is not a digit of the base.
static bool take_number(const char** s, uint32_t* out, uint8_t base) {
  const char* p = *s;
  uint32_t n = 0;
  bool any = false;
  for (; *p != '\0' && *p != ' '; ++p) {
    int d = hex_digit(*p);
    if (d < 0 || d >= base) return false;
    n = n * base + (uint32_t)d;
    if (n > STEP_MAX_US) return false;
    any = true;
  }
  if (!any) return false;
  if (*p == ' ') ++p;
  *s = p;
  *out = n;
  return true;
}

// "<first> <cols> <rows> <step_us>" with first in the base given; false when anything is off.
static bool parse_lcd(const char* s, uint8_t base, uint32_t* first, uint32_t* cols, uint32_t* rows,
                      uint32_t* step) {
  if (!take_number(&s, first, base) || !take_number(&s, cols, 10) || !take_number(&s, rows, 10) ||
      !take_number(&s, step, 10) || *s != '\0') {
    return false;
  }
  if (*step < STEP_MIN_US || *step > STEP_MAX_US) return false;
  if (*cols < 8 || *cols > 40) return false;
  if (*rows != 1 && *rows != 2 && *rows != 4) return false;
  if (*rows == 4 && *cols > 20) return false;
  return true;
}

static void handle_line(char* text) {
  if (strcmp(text, "HELLO") == 0) {
    Serial.println(F("LA-RIG 1"));
  } else if (strcmp(text, "STOP") == 0) {
    session++;
    mode = MODE_IDLE;
    extra = 0;
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
  } else if (strncmp(text, "LCDP ", 5) == 0) {
    uint32_t bus, cols, rows, step;
    if (parse_lcd(text + 5, 10, &bus, &cols, &rows, &step) && (bus == 4 || bus == 8)) {
      step_us = step;
      start_lcd(bus == 8 ? LCD_PAR8 : LCD_PAR4, (uint8_t)cols, (uint8_t)rows);
      Serial.println(F("OK"));
    } else {
      Serial.println(F("ERR"));
    }
  } else if (strncmp(text, "LCDI ", 5) == 0) {
    uint32_t addr, cols, rows, step;
    if (parse_lcd(text + 5, 16, &addr, &cols, &rows, &step) && addr >= 0x08 && addr <= 0x77) {
      step_us = step;
      lcd_addr = (uint8_t)addr;
      start_lcd(LCD_I2C, (uint8_t)cols, (uint8_t)rows);
      Serial.println(F("OK"));
    } else {
      Serial.println(F("ERR"));
    }
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
  DDRB |= 0x1F;  // D8, D9, and D10..D12 for the upper data lines of an 8-bit HD44780 bus
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
    case MODE_LCD: {
      // One script a second: the next one starts one second after the last one started.
      uint32_t started = millis();
      lcd_script();
      while (mode == MODE_LCD && session == script_session && millis() - started < 1000) {
        poll_serial();
      }
      return;
    }
  }
  write_pins(value);
}
