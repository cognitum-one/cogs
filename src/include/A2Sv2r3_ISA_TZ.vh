//================================================================================================================================
//
//   Copyright (c) 2024 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         :  A2S Assembler
//
//   Description          :  A2S Instruction Set Architectue generating Hardware and Software constants
//
//================================================================================================================================
//
//   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
//================================================================================================================================
//
//   THIS FILE IS AUTO GENERATED !!!!
//
//   DO NOT MODIFY !!!!
//
//   Modify "A2Sv2r3_ISA.ods" spreadsheet and then run "ods2hdr.py"
//
//================================================================================================================================

// Primary Opcodes
localparam [5:0]
   PUTA     = 6'b000000,   // (D: x -- )(A: adr)   (M: M[adr] <- x)
   PUTB     = 6'b000001,   // (D: x -- )(B: adr)   (M: M[adr] <- x)
   PUTC     = 6'b000010,   // (D: x -- )(C: adr)   (M: M[adr] <- x)
   PUT      = 6'b000011,   // (D: x adr -- )       (M: M[adr] <- x)  -- double pop!!
   GETA     = 6'b000100,   // (D: -- x )(A: adr)   (M: x <- M[adr])
   GETB     = 6'b000101,   // (D: -- x )(B: adr)   (M: x <- M[adr])
   GETC     = 6'b000110,   // (D: -- x )(C: adr)   (M: x <- M[adr])
   GET      = 6'b000111,   // (D: adr -- x )       (M: x <- M[adr])
   PTAP     = 6'b001000,   // (D: x -- )(A: adr -- (adr + cellsize)) (M: M[adr] <- x )
   PTBP     = 6'b001001,   // (D: x -- )(B: adr -- (adr + cellsize)) (M: M[adr] <- x )
   PTCP     = 6'b001010,   // (D: x -- )(C: adr -- (adr + cellsize)) (M: M[adr] <- x )
   XCHG     = 6'b001011,   // (D: x1 adr -- x2)                      (M: x2 <- M[adr] <- x1 atomically)
   GTAP     = 6'b001100,   // (D: -- x )(A: adr -- (adr + cellsize)) (M: x  <- M[adr])
   GTBP     = 6'b001101,   // (D: -- x )(B: adr -- (adr + cellsize)) (M: x  <- M[adr])
   GTCP     = 6'b001110,   // (D: -- x )(C: adr -- (adr + cellsize)) (M: x  <- M[adr])
   res1     = 6'b001111,   // (D: -- ) reserved
   DECA     = 6'b010000,   // (D: -- )  (A: adr -- (adr - cellsize)
   DECB     = 6'b010001,   // (D: -- )  (B: adr -- (adr - cellsize)
   DECC     = 6'b010010,   // (D: -- )  (C: adr -- (adr - cellsize)
   SNB      = 6'b010011,   // (D: -- )  (P: skip to next bundle )
   res2     = 6'b010100,   // (D: -- ) reserved
   res3     = 6'b010101,   // (D: -- ) reserved
   res4     = 6'b010110,   // (D: -- ) reserved
   NOT      = 6'b010111,   // (D: x -- ~x)
   T2A      = 6'b011000,   // (D: x -- )(A: -- x)
   T2B      = 6'b011001,   // (D: x -- )(B: -- x)
   T2C      = 6'b011010,   // (D: x -- )(C: -- x)
   T2R      = 6'b011011,   // (D: x -- )(R: -- x)
   A2T      = 6'b011100,   // (D: -- x )(A: x -- x)
   B2T      = 6'b011101,   // (D: -- x )(B: x -- x)
   C2T      = 6'b011110,   // (D: -- x )(C: x -- x)
   R2T      = 6'b011111,   // (D: -- x )(R: x --  )
   NIP      = 6'b100000,   // (D: x1 x2 -- x2 )
   DROP     = 6'b100001,   // (D: x1 x2 -- x1 )
   DUP      = 6'b100010,   // (D: x -- x x  )
   OVER     = 6'b100011,   // (D: x1 x2 -- x1 x2 x1  )
   SWAP     = 6'b100100,   // (D: x1 x2 -- x2 x1      )
   ROT3     = 6'b100101,   // (D: x1 x2 x3 -- x2 x3 x1 )
   ROT4     = 6'b100110,   // (D: x1 x2 x3 x4 -- x2 x3 x4 x1)
   SEL      = 6'b100111,   // (D: x1 x2 flag  -- x3 = flag == FALSE ? x2 : x1 )
   ADD      = 6'b101000,   // (D: n1|u1 n2|u2 -- n3|u3 = n1|u1 + n2|u2 )
   SUB      = 6'b101001,   // (D: n1|u1 n2|u2 -- n3|u3 = n1|u1 - n2|u2 )
   SLT      = 6'b101010,   // (D: n1 n2 -- flag = (n1  < n2) ? TRUE : FALSE )
   ULT      = 6'b101011,   // (D: u1 u2 -- flag = (u1 u< u2) ? TRUE : FALSE )
   EQ       = 6'b101100,   // (D: x1 x2 -- flag = (x1 == x2) ? TRUE : FALSE )
   XOR      = 6'b101101,   // (D: x1 x2 -- x3 = x1 ^ x2)
   IOR      = 6'b101110,   // (D: x1 x2 -- x3 = x1 | x2)
   AND      = 6'b101111,   // (D: x1 x2 -- x3 = x1 & x2)
   ZERO     = 6'b110000,   // (D: -- 0 )
   ONE      = 6'b110001,   // (D: -- 1 )
   CO       = 6'b110010,   // (D: -- )(R:rtn-addr -- P+(*1))  (P: u -- rtn_addr) *1: P is advanced to next 2 byte boundary
   RTN      = 6'b110011,   // (D: -- )(R:rtn-addr -- )        (P: u -- rtn-addr)
   NOP      = 6'b110100,   // (D: -- )
   PFX      = 6'b110101,   // (D: -- )                        (P: u1 -- u2 = u1 + 2)
   LIT      = 6'b110110,   // (D: -- x )                      (P: u1 -- u2 = u1 + 2)
   EXT      = 6'b110111,   // variable, see extension function coding
   res5     = 6'b111000,   // (D: -- ) reserved
   CALL     = 6'b111001,   // (D: -- )(R: -- P+(*1))          (P: u1 -- u2 = u1 + offset)   *1
   JN       = 6'b111010,   // (D: n -- )                      (P: u1 -- u2 = u1 + (n<0  ? offset : 2))
   JZ       = 6'b111011,   // (D: x -- )                      (P: u1 -- u2 = u1 + (x==0 ? offset : 2))
   JANZ     = 6'b111100,   // (D: -- )(A: u -- u-1 )          (P: u1 -- u2 = u1 + (A<>0 ? offset : 2))
   JBNZ     = 6'b111101,   // (D: -- )(B: u -- u-1 )          (P: u1 -- u2 = u1 + (B<>0 ? offset : 2))
   JCNZ     = 6'b111110,   // (D: -- )(C: u -- u-1 )          (P: u1 -- u2 = u1 + (C<>0 ? offset : 2))
   JMP      = 6'b111111;   // (D: -- )                        (P: u1 -- u2 = u1 + offset)

// Filler Opcodes
localparam [1:0]
   MT       = 2'b00, // (D: -- )   NEVER EXECUTED!! Often skipped over with no time penalty
   CALL2    = 2'b01, // (D: -- )  (R: -- P+(*1))        (P: u1 -- u2 = u1 + offset)   *1
   LIT2     = 2'b10, // (D: -- x )                      (P: u1 -- u2 = u1 + 2)
   RTN2     = 2'b11; // (D: -- )  (R: rtn-addr -- )     (P: u1 -- rtn-addr)

// Extensions Opcodes
localparam [15:0]
   IORC     = 16'b000?????????????, // (D: -- x )     (IO: x  <- IO[io-addr] <- 0            )  Read and Clear
   IORD     = 16'b001?????????????, // (D: -- x )     (IO: x  <- IO[io-addr]                 )  Read
   IOWR     = 16'b010?????????????, // (D: x -- )     (IO:       IO[io-addr] <- x            )  Write
   IOSW     = 16'b011?????????????, // (D: x1 -- x2 ) (IO: x2 <- IO[io-addr] <- x1           )  Swap
   RORI     = 16'b10000000????????, // (D: u1|n1 -- u2|n2 = u1|n1 <>> imm8 )  Rotate  Shift Right Immediate
   ROLI     = 16'b10000001????????, // (D: u1|n1 -- u2|n2 = u1|n1 <<> imm8 )  Rotate  Shift Left  Immediate
   LSRI     = 16'b10000010????????, // (D: n1    --  n2   = n1    >>  imm8 )  Logical Shift Right Immediate
   LSLI     = 16'b10000011????????, // (D: n1    --  n2   = n1    <<  imm8 )  Logical Shift Leftt Immediate
   ASRI     = 16'b10000100????????, // (D: n1    --  n2   = n1    >>> imm8 )  Arithmetic Shift Right Immediate
   CHARI    = 16'b10000101????????, // (D: x1    --  n2   = 0x000000ff & (x1 >> (8* imm8 & 0x3)) Extract Char from x1 with immediate index 3,2,1 or 0
   CC       = 16'b10001100????????, // (D: -- flag ) if Condition-Code[imm8] flag = TRUE else flag = FALSE
   WASM     = 16'b1110????????????, // ( See WASM-SIMD ) if no short immediates bits 11..8 are don’t care
   ROR      = 16'b1111000000000000, // (D: x1 x2 -- x3 =  x1 <>> x2 ) Rotate  Shift    Right Relative
   ROL      = 16'b1111000000000001, // (D: x1 x2 -- x3 =  x1 <<> x2 ) Rotate  Shift    Left  Relative
   LSR      = 16'b1111000000000010, // (D: x1 x2 -- x3 =  x1 >>  x2 ) Logical Shift    Right Relative
   LSL      = 16'b1111000000000011, // (D: x1 x2 -- x3 =  x1 <<  x2 ) Logical Shift    Left Relative
   ASR      = 16'b1111000000000100, // (D: x1 x2 -- x3 =  x1 >>> x2 ) Arithmetic Shift Right Relative
   CHAR     = 16'b1111000000000101, // (D: x1 x2 -- x3 =  0x000000ff & (x1 >> (8* (0x00000003 & x2)) Extract Char from x2 using index x1
   POPC     = 16'b1111000000010000, // (D: n1 n2 -- n3 = number-of-bits-set-in        (n1 & ~n2) )
   EXCS     = 16'b1111000000010001, // (D: n1 n2 -- n3 = excess number-of-bits-set-in (n1 & ~n2) ) positive if more 1s than 0s else a negative value
   CLZ      = 16'b1111000000010010, // (D: n1 n2 -- n3 = number-of-leading-zeros-in   (n1 & ~n2) )
   CTZ      = 16'b1111000000010011, // (D: n1 n2 -- n3 = number-of-trailing-zeros-in  (n1 & ~n2) )
   BSWP     = 16'b1111000000100000, // (D: x1    -- x2 = {x1[7:0],x1[15:8],x1[23:16],x1[31:24]} )
   BREV     = 16'b1111000000100001, // (D: x1    -- x2 =  x1[0:31]                              )  Bit reversal in cell
   SHFL     = 16'b1111000000100010, // (D: x1 x2 -- x3 x4 = {x1[31],x2[31]......x1[0],x2[0]}    )  A perfect shuffle
   SPILR    = 16'b1111000000110000, // (D: -- )( Spill from R-stack to Data Memory)
   SPILD    = 16'b1111000000110001, // (D: -- )( Spill from D-stack to Data Memory)
   FILLR    = 16'b1111000000110010, // (D: -- )( Fill  from Data Memory to R-stack)
   FILLD    = 16'b1111000000110011, // (D: -- )( Fill  from Data Memory to D-stack)
   MPY      = 16'b11110000010000??, // (D: n1 n2 -- n3 n4)   lower_product upper_product -- default ss
   MPH      = 16'b11110000010001??, // (D: n1 n2 -- n3 )     upper_product               -- default ss
   MPL      = 16'b11110000010010??, // (D: n1 n2 -- u3 )     lower_product               -- default ss
   DIV      = 16'b11110000010100??, // (D: n1 n2 -- n3 n4 )  remainder     quotient      -- default ss
   REM      = 16'b11110000010101??, // (D: n1 n2 -- n3 )     remainder                   -- default ss
   QUO      = 16'b11110000010110??, // (D: n1 n2 -- n3 )     quotient                    -- default ss
   A2SM     = 16'b11111001????????, // ( See A2SM    definitions)
   SPFPU    = 16'b11111101????????; // ( See Single Precision FPU definitions)

// Single_Precision_FPU Opcodes
localparam [7:0]
   FMAD     = 8'b00000111, // (D: f1 f2 f3 -- f4 = rm(f3 *  f2  + f1) )
   FMSB     = 8'b00001111, // (D: f1 f2 f3 -- f4 = rm(f3 *  f2  - f1) )
   FMNA     = 8'b00010111, // (D: f1 f2 f3 -- f4 = rm(f3 * -f2  + f1) )
   FMNS     = 8'b00011111, // (D: f1 f2 f3 -- f4 = rm(f3 * -f2  - f1) )
   FMUL     = 8'b00100111, // (D:    f1 f2 -- f3 = rm(f2 *  f1      ) )
   FADD     = 8'b00101111, // (D:    f1 f2 -- f3 = rm(f2 *  1.0 + f1) )
   FSUB     = 8'b00110111, // (D:    f1 f2 -- f3 = rm(f2 * -1.0 + f1) )
   FRSB     = 8'b00111111, // (D:    f1 f2 -- f3 = rm(f2 *  1.0 - f1) )
   FDIV     = 8'b01000111, // (D:    f1 f2 -- f3 = rm(f1 / f2) )
   FREM     = 8'b01001111, // (D:    f1 f2 -- f3 = rm(f1 - (f2*n)) ) where n is nearest to f1/f2
   FMOD     = 8'b01010111, // (D:    f1 f2 -- f3 = rm(f1 - (f2*n)) ) where n is closest & not greater than f1/f2
   FSQR     = 8'b01011111, // (D:       f1 -- f2 = rm(square-root(f)) )
   FF2S     = 8'b01100111, // (D:       f1 -- n1 = rm( sfix(f)) )    # float to signed
   FS2F     = 8'b01101111, // (D:       n1 -- f1 = rm(float(n)) )    # signed to float
   FF2U     = 8'b01110111, // (D:       f1 -- u1 = rm( ufix(f)) )    # float to unsigned
   FU2F     = 8'b01111111, // (D:       u1 -- f1 = rm(float(u)) )    # unsigned to float
   FCLT     = 8'b10100001, // (D:    f1 f2 -- flag = if  f1 <  f2  then TRUE  else FALSE)
   FCEQ     = 8'b10100010, // (D:    f1 f2 -- flag = if  f1 =  f2  then TRUE  else FALSE)
   FCLE     = 8'b10100011, // (D:    f1 f2 -- flag = if  f1 <= f2  then TRUE  else FALSE)
   FCGT     = 8'b10100100, // (D:    f1 f2 -- flag = if  f1 >  f2  then TRUE  else FALSE)
   FCNE     = 8'b10100101, // (D:    f1 f2 -- flag = if  f1 <> f2  then TRUE  else FALSE)
   FCGE     = 8'b10100110, // (D:    f1 f2 -- flag = if  f1 <= f2  then TRUE  else FALSE)
   FMAX     = 8'b10101100, // (D:    f1 f2 -- f3   = if  f1 >  f2  then f1    else f2   )
   FMIN     = 8'b10101001, // (D:    f1 f2 -- f3   = if  f1 >  f2  then f2    else f1   )
   FSAT     = 8'b10110001, // (D:    f1 f2 -- f3   = if  f1 <  0.0 then 0.0   else min(f1,f2))
   FNAN     = 8'b11111000, // (D:       f1 -- f2   = if  f1 =  NaN then FALSE else f1   )
   FABS     = 8'b11111100, // (D:       f1 -- f2   = |f1| )
   FCHS     = 8'b11111110; // (D:       f1 -- f2   = -f1  ) simply invert msb!

// FPU_Num_Operands Opcodes
localparam [15:0]
   FP3      = 16'b11111101?00?????, // FPU 3 operand functions
   FP2      = 16'b11111101?01?????, // FPU 2 operand functions
   FP1      = 16'b11111101?11?????; // FPU 1 operand functions

// Signedness Opcodes
localparam [1:0]
   UU       = 2'b00, // lhs is unsigned,  rhs is unsigned
   US       = 2'b01, // lhs is unsigned,  rhs is   signed
   SU       = 2'b10, // lhs is   signed,  rhs is unsigned
   SS       = 2'b11; // lhs is   signed,  rhs is   signed  DEFAULT

// Rounding Opcodes
localparam [2:0]
   NEAR     = 3'b000,   // round to nearest
   DOWN     = 3'b001,   // round to - infinity
   UP       = 3'b010,   // round to + infinity
   TRUNC    = 3'b011,   // truncate
   MAX      = 3'b100,   // NOT CURRENTLY IMPLEMENTED
   DEF      = 3'b111;   // DEFAULT: Use Floating Point Control\Status Register to define rounding mode

// WASM_SIMD Opcodes
localparam [7:0]
   v128_load                        = 8'b00000000, // (V: -- v ) (D: adr -- ) (M: v <- WM[adr])
   v128_store                       = 8'b00001011, // (V: v -- ) (D: adr -- ) (M: WM[adr] <- v)
   v128_const                       = 8'b00001100, // (V: v0 – v0 = v0 << 32 | Si ) RETARGETED :  Need repeat 4 times to emulate ‘v128.const’
   i8x16_shuffle                    = 8'b00001101, // EMULATED [ v128_const i8x16_swoozle ]
   i8x16_swizzle                    = 8'b00001110, // EMULATED [ v128_load  i8x16_swoozle ] RETARGETED : swizzle becomes swoozle
   i8x16_splat                      = 8'b00001111, // scalar
   i16x8_splat                      = 8'b00010000, // scalar
   i32x4_splat                      = 8'b00010001, // scalar
   i8x16_extract_lane_s             = 8'b00010101, // (D: -- x = sign_extend_8bit(SIMD.V0 >> ( 8 * imm4)) )
   i8x16_extract_lane_u             = 8'b00010110, // (D: -- x = zero_extend_8bit(SIMD.V0 >> ( 8 * imm4)) )
   i8x16_replace_lane               = 8'b00010111, // (D: x --                     (SIMD.V0[lane] <= x[7:0])  << Use to emulate v128.load8_lane
   i16x8_extract_lane_s             = 8'b00011000, // (D: -- x = sign_extend_16bit (SIMD.V0 >> (16 * imm4)))
   i16x8_extract_lane_u             = 8'b00011001, // (D: -- x = zero_extend_16bit (SIMD.V0 >> (16 * imm4)))
   i16x8_replace_lane               = 8'b00011010, // (D: x -- )                   (SIMD.V0[lane] <= x[15:0]) << Use to emulate v128.load16_lane
   i32x4_extract_lane               = 8'b00011011, // (D: -- x =                    SIMD.V0 >> (32 * imm2)  )
   i32x4_replace_lane               = 8'b00011100, // (D: x -- )                   (SIMD.V0[lane] <= x[31:0]) << Use to emulate v128.load32_lane
   i8x16_eq                         = 8'b00100011, // (V: foreach i8 (v1 v0 – v2 = v1.i8 == v0.i8 ? TRUE.i8 : FALSE.i8))
   i8x16_lt_s                       = 8'b00100101, // (V: foreach i8 (v1 v0 – v2 = v1.i8 <  v0.i8 ? TRUE.i8 : FALSE.i8))
   i8x16_lt_u                       = 8'b00100110, // (V: foreach i8 (v1 v0 – v2 = v1.i8 u< v0.i8 ? TRUE.i8 : FALSE.i8))
   i16x8_eq                         = 8'b00101101, // (V: foreach i16 (v1 v0 – v2 = v1.i16 == v0.i16 ? TRUE.i16 : FALSE.i16))
   i16x8_lt_s                       = 8'b00101111, // (V: foreach i16 (v1 v0 – v2 = v1.i16 <  v0.i16 ? TRUE.i16 : FALSE.i16))
   i16x8_lt_u                       = 8'b00110000, // (V: foreach i16 (v1 v0 – v2 = v1.i16 u< v0.i16 ? TRUE.i16 : FALSE.i16))
   i32x4_eq                         = 8'b00110111, // (V: foreach i32 (v1 v0 – v2 = v1.i32 == v0.i32 ? TRUE.i32 : FALSE.i32))
   i32x4_lt_s                       = 8'b00111001, // (V: foreach i32 (v1 v0 – v2 = v1.i32 <  v0.i32 ? TRUE.i32 : FALSE.i32))
   i32x4_lt_u                       = 8'b00111010, // (V: foreach i32 (v1 v0 – v2 = v1.i32 u< v0.i32 ? TRUE.i32 : FALSE.i32))
   v128_not                         = 8'b01001101, // (V: v -- ~v )
   v128_and                         = 8'b01001110, // (V: v2 v1 -- v3 =  v2 & v1 )
   v128_andnot                      = 8'b01001111, // (V: v2 v1 -- v3 = ~v2 & v1 )   -- verify
   v128_or                          = 8'b01010000, // (V: v2 v1 -- v3 =  v2 | v1 )
   v128_xor                         = 8'b01010001, // (V: v2 v1 -- v3 =  v2 ^ v1 )
   v128_bitselect                   = 8'b01010010, // (V: v3 v2 v1 -- v4 =  v3 & v2 | ~v3 v1 )  -- verify
   v128_any_true                    = 8'b01010011, // (V: v1 -- v2 = v1 == 0 ? 0 : 1)
   v128_load8_lane                  = 8'b01010100, // EMULATED [ i8x16_extract_lane i8x16_replace_lane ]
   v128_load16_lane                 = 8'b01010101, // EMULATED [ i16x8_extract_lane i16x8_replace_lane ]
   v128_load32_lane                 = 8'b01010110, // EMULATED [ i32x4_extract_lane i32x4_replace_lane ]
   v128_store8_lane                 = 8'b01011000, // EMULATED [ v128_load simd_dup i8x16_extract_lane i8x16_replace_lane v128_store]
   v128_store16_lane                = 8'b01011001, // EMULATED [ v128_load simd_dup i16x8_extract_lane i16x8_replace_lane v128_store]
   v128_store32_lane                = 8'b01011010, // EMULATED [ v128_load simd_dup i32x4_extract_lane i32x4_replace_lane v128_store]
   v128_load32_zero                 = 8'b01011100, // (D: x -- ) (V: -- v = 0 | x )
   i8x16_abs                        = 8'b01100000, // (V: foreach i8 (v1 -- v2 = sign(v1.i8) ? 0 – vi.i8 : vi.i8
   i8x16_neg                        = 8'b01100001, // (V: foreach i8 (v1 -- v2 = 0 – v1.i8  )
   i8x16_popcnt                     = 8'b01100010, // (V: foreach i8 (v1 -- v2 = popcount(v1.i8) )
   i8x16_all_true                   = 8'b01100011, // (V: v -- )(D: -- x = TRUE for i in range (16): x    = v.i8[i] == 0 ? FALSE : x )
   i8x16_bitmask                    = 8'b01100100, // (V: v -- )(D: -- x = 0    for i in range (16): x[i] = v.i8[i] <  0 )
   i8x16_narrow_i16x8_s             = 8'b01100101, // (V: foreach i8 (v1 -- v2 = |v1.i8| )
   i8x16_narrow_i16x8_u             = 8'b01100110, // (V: foreach i8 (v1 -- v2 = |v1.i8| )
   i8x16_shl                        = 8'b01101011, // -
   i8x16_shr_s                      = 8'b01101100, // -
   i8x16_shr_u                      = 8'b01101101, // -
   i8x16_add                        = 8'b01101110, // -
   i8x16_add_sat_s                  = 8'b01101111, // -
   i8x16_add_sat_u                  = 8'b01110000, // -
   i8x16_sub                        = 8'b01110001, // -
   i8x16_sub_sat_s                  = 8'b01110010, // -
   i8x16_sub_sat_u                  = 8'b01110011, // -
   i8x16_min_s                      = 8'b01110110, // -
   i8x16_min_u                      = 8'b01110111, // -
   i8x16_max_s                      = 8'b01111000, // -
   i8x16_max_u                      = 8'b01111001, // -
   i8x16_avgr_u                     = 8'b01111011, // -
   i16x8_extadd_pairwise_i8x16_s    = 8'b01111100, // -
   i16x8_extadd_pairwise_i8x16_u    = 8'b01111101, // -
   i32x4_extadd_pairwise_i16x8_s    = 8'b01111110, // -
   i32x4_extadd_pairwise_i16x8_u    = 8'b01111111, //
   i16x8_abs                        = 8'b10000000, // -
   i16x8_neg                        = 8'b10000001, // -
   i16x8_all_true                   = 8'b10000011, // -
   i16x8_bitmask                    = 8'b10000100, // -
   i16x8_narrow_i32x4_s             = 8'b10000101, //
   i16x8_narrow_i32x4_u             = 8'b10000110, //
   i16x8_extend_dbl_i8x16_s         = 8'b10000111, // i16x8_extend_low_i8x16_s  = i16x8_extend_dbl_i8x16_s, simd_nip || i16x8_extend_high_i8x16_s = i16x8_extend_dbl_i8x16_s, simd_drop
   i16x8_extend_dbl_i8x16_u         = 8'b10001001, // i16x8_extend_low_i8x16_u  = i16x8_extend_dbl_i8x16_u, simd_nip || i16x8_extend_high_i8x16_u = i16x8_extend_dbl_i8x16_u, simd_drop
   i16x8_shl                        = 8'b10001011, // -
   i16x8_shr_s                      = 8'b10001100, // -
   i16x8_shr_u                      = 8'b10001101, // -
   i16x8_add                        = 8'b10001110, // -
   i16x8_add_sat_s                  = 8'b10001111, // -
   i16x8_add_sat_u                  = 8'b10010000, // -
   i16x8_sub                        = 8'b10010001, // -
   i16x8_sub_sat_s                  = 8'b10010010, // -
   i16x8_sub_sat_u                  = 8'b10010011, // -
   i16x8_mul                        = 8'b10010101, // -
   i16x8_min_s                      = 8'b10010110, // -
   i16x8_min_u                      = 8'b10010111, // -
   i16x8_max_s                      = 8'b10011000, // -
   i16x8_max_u                      = 8'b10011001, // -
   i16x8_avgr_u                     = 8'b10011011, // -
   i16x8_extmul_dbl_i8x16_s         = 8'b10011100, // i16x8_extmul_low_i8x16_s  = i16x8_extmul_dbl_i8x16_s, simd_nip || i16x8_extmul_high_i8x16_s = i16x8_extmul_dbl_i8x16_s, simd_drop
   i16x8_extmul_dbl_i8x16_u         = 8'b10011110, // i16x8_extmul_low_i8x16_u  = i16x8_extmul_dbl_i8x16_u, simd_nip || i16x8_extmul_high_i8x16_u = i16x8_extmul_dbl_i8x16_u, simd_drop
   i32x4_abs                        = 8'b10100000, // -
   i32x4_neg                        = 8'b10100001, // -
   i32x4_all_true                   = 8'b10100011, // -
   i32x4_bitmask                    = 8'b10100100, // -
   i32x4_extend_dbl_i16x8_s         = 8'b10100111, // i32x4_extend_low_i16x8_s  = i32x4_extend_dbl_i16x8_s, simd_nip || i32x4_extend_high_i16x8_s = i32x4_extend_dbl_i16x8_s, simd_drop
   i32x4_extend_dbl_i16x8_u         = 8'b10101001, // i32x4_extend_low_i16x8_u  = i32x4_extend_dbl_i16x8_u, simd_nip || i32x4_extend_high_i16x8_u = i32x4_extend_dbl_i16x8_u, simd_drop
   i32x4_shl                        = 8'b10101011, // -
   i32x4_shr_s                      = 8'b10101100, // -
   i32x4_shr_u                      = 8'b10101101, // -
   i32x4_add                        = 8'b10101110, // -
   i32x4_sub                        = 8'b10110001, // -
   i32x4_mul                        = 8'b10110101, // -
   i32x4_min_s                      = 8'b10110110, // -
   i32x4_min_u                      = 8'b10110111, // -
   i32x4_max_s                      = 8'b10111000, // -
   i32x4_max_u                      = 8'b10111001, // -
   i32x4_dot_i16x8_s                = 8'b10111010, // -
   i32x4_extmul_dbl_i16x8_s         = 8'b10111100, // i32x4_extmul_low_i16x8_s  = i32x4_extmul_dbl_i16x8_s, simd_nip || i32x4_extmul_high_i16x8_s = i32x4_extmul_dbl_i16x8_s, simd_drop
   i32x4_extmul_dbl_i16x8_u         = 8'b10111110; // i32x4_extmul_low_i16x8_s  = i32x4_extmul_dbl_i16x8_u, simd_nip || i32x4_extmul_high_i16x8_s = i32x4_extmul_dbl_i16x8_u, simd_drop

// A2S_SIMD Opcodes
localparam [7:0]
   simd_push                        = 8'b0000????, // (v3 v2 v1 v0 -- v2 v1 v0 MR[mode])
   simd_replace                     = 8'b0001????, // (v3 v2 v1 v0 -- v3 v2 v1 MR[mode])
   simd_popcount                    = 8'b0010????, // (EN v3 v2 v1 v0 -- {EN v3 v2 v1 v0} + MR[mode])           EN is Carry out  (max loopcount is therefore 31)
   simd_xnor_popcount               = 8'b0011????, // (EN v3 v2 v1 v0 -- {EN v3 v2 v1 v0} + (iS[0] ~^ MR[mode]) EN is Carry out  (max loopcount is therefore 31)
   simd_load_EN                     = 8'b0100????, // (EN = MR[mode])
   simd_store_EN                    = 8'b0101????, // (MR[mode] = EN)
   simd_store_masked                = 8'b0110????, // (v3 v2 v1 v0 -- v3 v3 v2 v1) (MW[mode]@EN = v0)
   simd_store                       = 8'b0111????, // (v3 v2 v1 v0 -- v3 v3 v2 v1) (MW[mode]    = v0)
   simd_bit_add                     = 8'b10000000, // (v2 v1 v0 -- v1 v0)       Bit-serial-add:      carry, sum = fulladd (v0 v1 v2)      v1 = carry, v0 = sum
   simd_bit_sub                     = 8'b10000001, // (v2 v1 v0 -- v1 v0)       Bit-serial-sub:      carry, sum = fullsub (v0 v1 v2)      v1 = carry, v0 = sum
   simd_bit_inc                     = 8'b10000010, // (   v1 v0 -- v1 v0)       Bit-serial-inc:      carry, sum = halfadd (v0 v1)         v1 = carry, v0 = sum
   simd_bit_dec                     = 8'b10000011, // (   v1 v0 -- v1 v0)       Bit-serial-dec:      carry, sum = halfsub (v0 v1)         v1 = carry, v0 = sum
   simd_bit_add_sub                 = 8'b10000100, // (v3 v2 v1 v0 -- v1 v0)    Bit-serial-add/sub:  carry, sum = v3 ? Fulladd : Fullsub  v1 = carry, v0 = sum
   simd_bit_inc_dec                 = 8'b10000101, // (v3 v2 v1 v0 -- v1 v0)    Bit-serial-inc/dec:  carry, sum = v3 ? Halfadd : Halfsub  v1 = carry, v0 = sum
   simd_merge                       = 8'b10000110, // (v3 v2 v1 v0 -- v3 v3 v2 (EN ? v0 : v1))
   simd_and                         = 8'b10010000, // (v3 v2 v1 v0 -- v3 v3 v2 (v1 &  v0))
   simd_bic                         = 8'b10010001, // (v3 v2 v1 v0 -- v3 v3 v2 (v1 & ~v0))
   simd_ior                         = 8'b10010010, // (v3 v2 v1 v0 -- v3 v3 v2 (v1 |  v0))
   simd_xor                         = 8'b10010011, // (v3 v2 v1 v0 -- v3 v3 v2 (v1 ^  v0))
   simd_not                         = 8'b10010100, // (v3 v2 v1 v0 -- v3 v2 v1 ~v0)
   simd_swap                        = 8'b10011000, // (v3 v2 v1 v0 -- v3 v2 v0 v1)
   simd_nip                         = 8'b10100000, // (v3 v2 v1 v0 -- v3 v3 v2 v0)
   simd_drop                        = 8'b10100001, // (v3 v2 v1 v0 -- v3 v3 v2 v1)
   simd_dup                         = 8'b10100010, // (v3 v2 v1 v0 -- v2 v1 v0 v0)
   simd_over                        = 8'b10100011, // (v3 v2 v1 v0 -- v2 v1 v0 v1)
   simd_rotate3                     = 8'b10100100, // (v3 v2 v1 v0 -- v2 v1 v0 v2)
   simd_rotate4                     = 8'b10100101, // (v3 v2 v1 v0 -- v2 v1 v0 v3)
   simd_unrotate4                   = 8'b10100110, // (v3 v2 v1 v0 -- v0 v3 v2 v1)
   simd_zero                        = 8'b10110000, // (v3 v2 v1 v0 -- v2 v1 v0  0)
   simd_ones                        = 8'b10110001, // (v3 v2 v1 v0 -- v2 v1 v0 -1)
   simd_clear                       = 8'b10110010, // (v3 v2 v1 v0 --  0  0  0  0)
   simd_load_scalar                 = 8'b11000000, // (v3 v2 v1 v0 -- v2 v1 v0 T0)
   simd_store_scalar                = 8'b11000001, // (v3 v2 v1 v0 -- v3 v3 v2 v1)
   simd_set_EN                      = 8'b11010000, // (EN = T0[0])
   simd_read_EN                     = 8'b11010001, // (v3 v2 v1 v0 -- v2 v1 v0 EN)
   simd_write_EN                    = 8'b11010010, // (v3 v2 v1 v0 -- v3 v3 v2 v1)(EN = v0)
   simd_flip_EN                     = 8'b11010011, // (EN = ~EN)
   simd_select                      = 8'b11111111, // (v3 v2 v1 v0 -- v3 v3 v2 (iS == FALSE) ? v0 : v1)
   simd_orthogonal_sum              = 8'b11100000, // ({EN,v3,v2,v1,v0} -- (v0 = Corner-turned 16 sums of 8 x 5-bit values from popcount))
   simd_vector_sum                  = 8'b11100001; // (v3 v2 v1 v0 – v3 v3 v2 v1) (S: sum of 16 8-bit values from V0)

// SIMD_Addressing_modes Opcodes
localparam [3:0]
   WINC     = 4'b0000,  // (W: w -- w+16) (WM: adr = w)    Post-increment w by 16
   WPTR     = 4'b0001,  // (W: w -- w   ) (WM: adr = w)    Pointer w
   WDEC     = 4'b0010,  // (W: w -- w-16) (WM: adr = w-16) Pre-decrement w by 16
   XINC     = 4'b0100,  // (W: x -- x+16) (WM: adr = x)    Post-increment x by 16
   XPTR     = 4'b0101,  // (W: x -- x   ) (WM: adr = x)    Pointer x
   XDEC     = 4'b0110,  // (W: x -- x-16) (WM: adr = x-16) Pre-decrement x by 16
   YINC     = 4'b1000,  // (W: y -- y+16) (WM: adr = y)    Post-increment y by 16
   YPTR     = 4'b1001,  // (W: y -- y   ) (WM: adr = y)    Pointer y
   YDEC     = 4'b1010,  // (W: y -- y-16) (WM: adr = y-16) Pre-decrement y by 16
   ZINC     = 4'b1100,  // (W: z -- z+16) (WM: adr = z)    Post-increment z by 16
   ZPTR     = 4'b1101,  // (W: z -- z   ) (WM: adr = z)    Pointer z
   ZDEC     = 4'b1110;  // (W: z -- z-16) (WM: adr = z-16) Pre-decrement z by 16


`ifndef S2Vops
`define S2Vops 16'hE000,16'hE00B,16'hE00C,16'hE00F,16'hE010,16'hE011,16'hE?54,16'hE?55,16'hE?56,16'hE05C,16'hE06B,16'hE06C,16'hE06D,16'hE08B,16'hE08C,16'hE08D,16'hE0AB,16'hE0AC,16'hE0AD,16'hF9C0,16'hF9D0,16'hF9FF
`define V2Sops 16'hE053,16'hE?58,16'hE?59,16'hE?5A,16'hE063,16'hE064,16'hE083,16'hE084,16'hE0A3,16'hE0A4,16'hF9C1,16'hF9E1
`define VXOops 16'hE00D,16'hE00E,16'hE023,16'hE025,16'hE026,16'hE02D,16'hE02F,16'hE030,16'hE037,16'hE039,16'hE03A,16'hE04D,16'hE04E,16'hE04F,16'hE050,16'hE051,16'hE052,16'hE060,16'hE061,16'hE062,16'hE065,16'hE066,16'hE06E,16'hE06F,16'hE070,16'hE071,16'hE072,16'hE073,16'hE076,16'hE077,16'hE078,16'hE079,16'hE07B,16'hE07C,16'hE07D,16'hE07E,16'hE07F,16'hE080,16'hE081,16'hE085,16'hE086,16'hE087,16'hE089,16'hE08E,16'hE08F,16'hE090,16'hE091,16'hE092,16'hE093,16'hE095,16'hE096,16'hE097,16'hE098,16'hE099,16'hE09B,16'hE09C,16'hE09E,16'hE0A0,16'hE0A1,16'hE0A7,16'hE0A9,16'hE0AE,16'hE0B1,16'hE0B5,16'hE0B6,16'hE0B7,16'hE0B8,16'hE0B9,16'hE0BA,16'hE0BC,16'hE0BE,16'hF?,16'hF?,16'hF?,16'hF?,16'hF?,16'hF?,16'hF?,16'hF?,16'hF980,16'hF981,16'hF982,16'hF983,16'hF984,16'hF985,16'hF986,16'hF990,16'hF991,16'hF992,16'hF993,16'hF994,16'hF998,16'hF9A0,16'hF9A1,16'hF9A2,16'hF9A3,16'hF9A4,16'hF9A5,16'hF9A6,16'hF9B0,16'hF9B1,16'hF9B2,16'hF9D1,16'hF9D2,16'hF9D3,16'hF9E0
`endif