/*==============================================================================================================================

   Copyright © 2021 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : A2S v2
   Description          : ISA order encoding

================================================================================================================================
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
   Notes for Use and Synthesis
==============================================================================================================================*/

localparam [5:0]  // Order Encoding -------------------------------------------------------------------------------------------
                        //  where:  a-addr      aligned address
                        //          M[a-addr]   memory location
                        //          cellsize    the size, in address units, of a cell.
                        //          A,B & C are registers, R is the return stack
   PUTA  = 6'b 00_0000, //  0  !a      ( x -- )(A: a-addr)   (M: M[a-addr] <- x)
   PUTB  = 6'b 00_0001, //  1  !b      ( x -- )(B: a-addr)   (M: M[a-addr] <- x)
   PUTC  = 6'b 00_0010, //  2  !c      ( x -- )(C: a-addr)   (M: M[a-addr] <- x)
   PUT   = 6'b 00_0011, //  3  !       ( x a-addr -- n)      (M: M[a-addr] <- x)   ** only a single POP performed !!!
   GETA  = 6'b 00_0100, //  4  @a      ( -- x )(A: a-addr)   (M: x <- M[a-addr])
   GETB  = 6'b 00_0101, //  5  @b      ( -- x )(B: a-addr)   (M: x <- M[a-addr])
   GETC  = 6'b 00_0110, //  6  @c      ( -- x )(C: a-addr)   (M: x <- M[a-addr])
   GET   = 6'b 00_0111, //  7  @       ( a-addr -- n )       (M: x <- M[a-addr])
   PTAP  = 6'b 00_1000, //  8  !a+     ( x -- )(A: a-addr -- (a-addr + cellsize)) (M: M[a-addr] <- x )
   PTBP  = 6'b 00_1001, //  9  !b+     ( x -- )(B: a-addr -- (a-addr + cellsize)) (M: M[a-addr] <- x )
   PTCP  = 6'b 00_1010, // 10  !c+     ( x -- )(C: a-addr -- (a-addr + cellsize)) (M: M[a-addr] <- x )
   XCHG  = 6'b 00_1011, // 11  @!      ( x1 a-addr -- x2)                         (M: x2 <- M[a-addr] <- x1 atomically)
   GTAP  = 6'b 00_1100, // 12  @a+     ( -- x )(A: a-addr -- (a-addr + cellsize)) (M: x  <- M[a-addr])
   GTBP  = 6'b 00_1101, // 13  @b+     ( -- x )(B: a-addr -- (a-addr + cellsize)) (M: x  <- M[a-addr])
   GTCP  = 6'b 00_1110, // 14  @c+     ( -- x )(C: a-addr -- (a-addr + cellsize)) (M: x  <- M[a-addr])
   res1  = 6'b 00_1111, // 15  ---     reserved

   PTMA  = 6'b 01_0000, // 16  !-a     ( x -- )(A: a-addr -- (a-addr - cellsize)  (M: M[a-addr - cellsize] <- x)
   PTMB  = 6'b 01_0001, // 17  !-b     ( x -- )(B: a-addr -- (a-addr - cellsize)  (M: M[a-addr - cellsize] <- x)
   PTMC  = 6'b 01_0010, // 18  !-c     ( x -- )(C: a-addr -- (a-addr - cellsize)  (M: M[a-addr - cellsize] <- x)
   res2  = 6'b 01_0011, // 19  ---     reserved
   AZ    = 6'b 01_0100, // 20  a0      ( -- flag = A==0 ? TRUE : FALSE )
   BZ    = 6'b 01_0101, // 21  b0      ( -- flag = B==0 ? TRUE : FALSE )
   CZ    = 6'b 01_0110, // 22  c0      ( -- flag = C==0 ? TRUE : FALSE )
   DUP   = 6'b 01_0111, // 23  dup     ( x -- x x  )
   T2A   = 6'b 01_1000, // 24  >a      ( x -- )(A: -- x)
   T2B   = 6'b 01_1001, // 25  >b      ( x -- )(B: -- x)
   T2C   = 6'b 01_1010, // 26  >c      ( x -- )(C: -- x)
   T2R   = 6'b 01_1011, // 27  >r      ( x -- )(R: -- x)
   A2T   = 6'b 01_1100, // 28  a>      ( -- x )(A: x -- x)
   B2T   = 6'b 01_1101, // 29  b>      ( -- x )(B: x -- x)
   C2T   = 6'b 01_1110, // 30  c>      ( -- x )(C: x -- x)
   R2T   = 6'b 01_1111, // 31  r>      ( -- x )(R: x --  )

   AND   = 6'b 10_0000, // 32  &       ( x1 x2 -- x3 = bit-wise AND of x1, x2 )
   XOR   = 6'b 10_0001, // 33  ^       ( x1 x2 -- x3 = bit-wise XOR of x1, x2 )
   IOR   = 6'b 10_0010, // 34  |       ( x1 x2 -- x3 = bit-wise IOR of x1, x2 )
   res3  = 6'b 10_0011, // 35  ---     reserved
   OVER  = 6'b 10_0100, // 36  over    ( x1 x2 -- x1 x2 x1  )
   SWAP  = 6'b 10_0101, // 37  ><      ( x1 x2 -- x2 x1      )
   RT3   = 6'b 10_0110, // 38  ><<     ( x1 x2 x3 -- x2 x3 x1 )
   RT4   = 6'b 10_0111, // 39  ><<<    ( x1 x2 x3 x4 -- x2 x3 x4 x1)
   ADD   = 6'b 10_1000, // 40  +       ( n1|u1 n2|u2 -- n3|u3 = n1|u1 + n2|u2 )
   SUB   = 6'b 10_1001, // 41  -       ( n1|u1 n2|u2 -- n3|u3 = n1|u1 - n2|u2 )
   NIP   = 6'b 10_1010, // 42  nip     ( x1 x2 -- x2 )
   DROP  = 6'b 10_1011, // 43  drop    ( x1 x2 -- x1 )
   ZLT   = 6'b 10_1100, // 44  0<      ( n -- flag = localparam  [15:0] // Swap Bytes extension -------------------------------------------------------------------------------------
   ZEQ   = 6'b 10_1101, // 45  0=      ( x -- flag = x=0 ? TRUE : FALSE )
   ZGT   = 6'b 10_1110, // 46  0>      ( n -- flag = n>0 ? TRUE : FALSE )
   NOT   = 6'b 10_1111, // 47  ~       ( x -- ~x )

   res4  = 6'b 11_0000, // 48  ---     reserved
   CO    = 6'b 11_0001, // 49  co      ( -- )(R:rtn-addr -- P+(*1))  (P: u -- rtn_addr) *1: P is advanced to next 2 byte boundary
   NOP   = 6'b 11_0010, // 50  nop     ( -- )
   RTN   = 6'b 11_0011, // 51  rtn     ( -- )(R:rtn-addr -- )        (P: u -- rtn-addr)
   ZERO  = 6'b 11_0100, // 52  0       ( -- 0 )
   ONE   = 6'b 11_0101, // 53  1       ( -- 1 )
   LIT   = 6'b 11_0110, // 54  value   ( -- x )                      (P: u1 -- u2 = u1 + 2)
   CALL  = 6'b 11_0111, // 55  name    ( -- )(R: -- P+(*1))          (P: u1 -- u2 = u1 + offset)   *1
   PFX   = 6'b 11_1000, // 56  prefix  ( -- )                        (P: u1 -- u2 = u1 + 2)
   FN    = 6'b 11_1001, // 57  func    ( variable ) see function coding
   JMP   = 6'b 11_1010, // 58  ->      ( -- )                        (P: u1 -- u2 = u1 + offset)
   JZ    = 6'b 11_1011, // 59  0->     ( x -- )                      (P: u1 -- u2 = u1 + (x==0 ? offset : 2))
   JDANZ = 6'b 11_1100, // 60  a-->    ( -- )(A: u -- u-1 )          (P: u1 -- u2 = u1 + (A<>0 ? offset : 2))
   JDBNZ = 6'b 11_1101, // 61  b-->    ( -- )(B: u -- u-1 )          (P: u1 -- u2 = u1 + (B<>0 ? offset : 2))
   JDCNZ = 6'b 11_1110, // 62  c-->    ( -- )(C: u -- u-1 )          (P: u1 -- u2 = u1 + (C<>0 ? offset : 2))
   JN    = 6'b 11_1111; // 63  n->     ( n -- )                      (P: u1 -- u2 = u1 + (n<0  ? offset : 2))

localparam  [1:0] // 2-bit Order Encoding ---------------------------------------------------------------------------------------
   NOP2  = 2'b 00,      // Never Executed!!
   RTN2  = 2'b 01,      // 51  rtn     ( -- )  (R: rtn-addr -- )     (P: u1 -- rtn-addr)
   LIT2  = 2'b 10,      // 54  value   ( -- x )                      (P: u1 -- u2 = u1 + 2)
   CAL2  = 2'b 11;      // 55  name    ( -- )  (R: -- P+(*1))        (P: u1 -- u2 = u1 + offset)   *1

// 16-bit Function Encoding =====================================================================================================

localparam  [15:0] // I/O accesses
   IORC  = 16'b 000_?????_????_????, // $0000 + io-addr  ( -- x )    (IO: x  <- IO[io-addr] <- 0            )  Read and Clear
   IORD  = 16'b 001_?????_????_????, // $2000 + io-addr  ( -- x )    (IO: x  <- IO[io-addr]                 )  Read
   IOWR  = 16'b 010_?????_????_????, // $4000 + io-addr  ( x -- )    (IO:       IO[io-addr] <- x            )  Write
   IOSW  = 16'b 011_?????_????_????; // $6000 + io-addr  ( x1 -- x2 )(IO: x2 <- IO[io-addr] <- x1 atomically)  Swap

localparam  [2:0]  // IO OPCODES
   IORCop   =  3'b000,
   IORDop   =  3'b001,
   IOWRop   =  3'b010,
   IOSWop   =  3'b011;

localparam  [15:0] // Shift Operators -------------------------------------------------------------------------------------------
   SHFT  = 16'b 1?00_00??_????_????, // Any Shift

   SIMM  = 16'b 1000_00??_????_????, // Any Shift immediate
   LSLI  = 16'b 1000_0000_????_????, // $8000 + imm8     ( u1|n1 -- u2|n2 = u1|n1 <<  imm8 )     Logical Shift Left Immediate
   LSRI  = 16'b 1000_0001_????_????, // $8100 + imm8     ( u1|n1 -- u2|n2 = u1|n1 >>  imm8 )     Logical Shift Right Immediate
   ASLI  = 16'b 1000_0010_????_????, // $8200 + imm8     ( n1    --  n2   = n1    <<< imm8 )     Arithmetic Shift Left Immediate
   ASRI  = 16'b 1000_0011_????_????, // $8300 + imm8     ( n1    --  n2   = n1    >>> imm8 )     Arithmetic Shift Right Immediate

   SREL  = 16'b 1100_00??_????_????, // Any Shift relative
   LSLN  = 16'b 1100_0000_????_????, // $c000            ( u1|n1 u2 -- u3|n3 =  u1|n1 <<  u2 )   Logical Shift Left Relative
   LSRN  = 16'b 1100_0001_????_????, // $c000            ( u1|n1 u2 -- u3|n3 =  u1|n1 >>  u2 )   Logical Shift Right Relative
   ASLN  = 16'b 1100_0010_????_????, // $c000            ( n1    u2 -- n3    =  n1    <<< u2 )   Arithmetic Shift Left Relative
   ASRN  = 16'b 1100_0011_????_????; // $c000            ( n1    u2 -- n3    =  n1    >>> u2 )   Arithmetic Shift Right Relative

localparam  [15:0] // Condition Code extension----------------------------------------------------------------------------------
   CC    = 16'b 1000_0100_????_????; // $8400 +imm8      ( -- flag ) if Condition-Code[imm8] flag = TRUE else flag = FALSE

localparam  [15:0] // Population Count of XOR ----------------------------------------------------------------------------------
   POPC  = 16'b 1100_0100_0000_0000; // $c400            ( n1 n2 -- (number-of-bits-set-in(n1 ^ n2)))

localparam  [15:0] // Swap Bytes extension -------------------------------------------------------------------------------------
   SWBYT = 16'b 1100_0100_0000_0001; // $c401            ( x1 -- x2 )    x2 = {x1[7:0],x1[15:8],x1[23:16],x1[31:24]}

localparam  [15:0] // Swap Bytes extension -------------------------------------------------------------------------------------
   UNSL  = 16'b 1100_0100_0000_0010; // $c402            ( u1 u2 -- flag = ( u2 lower u1 ) ? TRUE : FALSE

localparam  [15:0] // Floating Point -------------------------------------------------------------------------------------------

   FM_u  = 16'b 1111_1110_00???_???, // Multiply add unit
   FMAD  = 16'b 1111_1110_00000_???, // $fe00 +rm        ( f1 f2 f3 -- f4 = rm(f3 *  f2  + f1) )
   FMSB  = 16'b 1111_1110_00001_???, // $fe08 +rm        ( f1 f2 f3 -- f4 = rm(f3 *  f2  - f1) )
   FMNA  = 16'b 1111_1110_00010_???, // $fe10 +rm        ( f1 f2 f3 -- f4 = rm(f3 * -f2  + f1) )
   FMNS  = 16'b 1111_1110_00011_???, // $fe18 +rm        ( f1 f2 f3 -- f4 = rm(f3 * -f2  - f1) )
   FMUL  = 16'b 1111_1110_00100_???, // $fe20 +rm        ( f1 f2    -- f3 = rm(f2 *  f1      ) )
   FADD  = 16'b 1111_1110_00101_???, // $fe28 +rm        ( f1 f2    -- f3 = rm(f2 *  1.0 + f1) )
   FSUB  = 16'b 1111_1110_00110_???, // $fe30 +rm        ( f1 f2    -- f3 = rm(f2 * -1.0 + f1) )
   FRSB  = 16'b 1111_1110_00111_???, // $fe38 +rm        ( f1 f2    -- f3 = rm(f2 *  1.0 - f1) )

   FD_u  = 16'b 1111_1110_010??_???, // Divide unit
   FDIV  = 16'b 1111_1110_01000_???, // $fe40 +rm        ( f1 f2 -- f3 = rm(f1 / f2) )
   FREM  = 16'b 1111_1110_01001_???, // $fe48 +rm        ( f1 f2 -- f3 = rm(f1 - (f2*n)) ) where n is nearest to f1/f2
   FMOD  = 16'b 1111_1110_01010_???, // $fe50 +rm        ( f1 f2 -- f3 = rm(f1 - (f2*n)) ) where n is closest & not greater than f1/f2
   FSQR  = 16'b 1111_1110_01011_???, // $fe58 +rm        ( f --  f2 = rm(square-root(f)) )

   FV_u  = 16'b 1111_1110_011?_????, // conVersion unit  (0=f2s, 1=s2f, 2=f2u, 3=u2f)
   FS2F  = 16'b 1111_1110_01100_???, // $fe60 +rm        ( n -- f = rm(float(n)) )
   FF2S  = 16'b 1111_1110_01101_???, // $fe68 +rm        ( f -- n = rm( sfix(f)) )    -- signed fix
   FU2F  = 16'b 1111_1110_01110_???, // $fe70 +rm        ( u -- f = rm(float(u)) )
   FF2U  = 16'b 1111_1110_01111_???, // $fe78 +rm        ( f -- u = rm( ufix(f)) )    -- unsigned fix

   FC_u  = 16'b 1111_1110_1000_0???, // Compare unit
   FCLT  = 16'b 1111_1110_1000_0001, // $fe81            ( f1 f2 -- flag ) if  f1 <  f2  then flag = TRUE else flag = FALSE
   FCEQ  = 16'b 1111_1110_1000_0010, // $fe82            ( f1 f2 -- flag ) if  f1 =  f2  then flag = TRUE else flag = FALSE
   FCLE  = 16'b 1111_1110_1000_0011, // $fe83            ( f1 f2 -- flag ) if  f1 <= f2  then flag = TRUE else flag = FALSE
   FCGT  = 16'b 1111_1110_1000_0100, // $fe84            ( f1 f2 -- flag ) if  f1 >  f2  then flag = TRUE else flag = FALSE
   FCNE  = 16'b 1111_1110_1000_0101, // $fe85            ( f1 f2 -- flag ) if  f1 <> f2  then flag = TRUE else flag = FALSE
   FCGE  = 16'b 1111_1110_1000_0110, // $fe86            ( f1 f2 -- flag ) if  f1 <= f2  then flag = TRUE else flag = FALSE

   FX_u  = 16'b 1111_1110_1000_10??, // Extras unit
   FMAX  = 16'b 1111_1110_1000_1000, // $fe88            ( f1 f2 -- f3 = (f1 > f2  ? f1  : f2) )
   FMIN  = 16'b 1111_1110_1000_1001, // $fe89            ( f1 f2 -- f3 = (f1 > f2  ? f2  : f1) )
   FSAT  = 16'b 1111_1110_1000_1010, // $fe8a            ( f1 f2 -- f3 = (if f1 < 0.0 then 0.0 else min(f1,f2)) )
   FX_a  = 16'b 1111_1110_1000_1011, // -- spare --

   FU_u  = 16'b 1111_1110_1000_11??, // Unary Op unit
   FABS  = 16'b 1111_1110_1000_1100, // $fe8c            ( f -- |f| )
   FNAN  = 16'b 1111_1110_1000_1101, // $fe8d            ( f -- (if f is NaN  then 0.0  else f) )
   FCHS  = 16'b 1111_1110_1000_1111; // $fe8f            ( f -- -f  ) simply invert msb!

localparam [2:0]  // Rounding Modes
   NEAR  =  3'b 000,
   UP    =  3'b 001,
   DOWN  =  3'b 010,
   TRNC  =  3'b 011,
// MAX   =  3'b 100,
   DFLT  =  3'b 111; // default

localparam  [15:0] // Multiply, Divide ------------------------------------------------------------------------------------------
   IMD   = 16'b 1111_1111_111?_????,

   IDIV  = 16'b 1111_1111_1110_????,
   DIV   = 16'b 1111_1111_1110_00??,
   DIVuu = 16'b 1111_1111_1110_0000, // $ffe0            ( u1 u2 -- u3 u4 )   remainder quotient
   DIVus = 16'b 1111_1111_1110_0001, // $ffe1            ( u1 n2 -- n3 n4 )   remainder quotient
   DIVsu = 16'b 1111_1111_1110_0010, // $ffe2            ( n1 u2 -- n3 n4 )   remainder quotient
   DIVss = 16'b 1111_1111_1110_0011, // $ffe3            ( n1 n2 -- n3 n4 )   remainder quotient   -- default
   REM   = 16'b 1111_1111_1110_01??,
   REMuu = 16'b 1111_1111_1110_0100, // $ffe4            ( u1 u2 -- u3 )      remainder
   REMus = 16'b 1111_1111_1110_0101, // $ffe5            ( u1 n2 -- n3 )      remainder
   REMsu = 16'b 1111_1111_1110_0110, // $ffe6            ( n1 u2 -- n3 )      remainder
   REMss = 16'b 1111_1111_1110_0111, // $ffe7            ( n1 n2 -- n3 )      remainder            -- default
   QUO   = 16'b 1111_1111_1110_10??,
   QUOuu = 16'b 1111_1111_1110_1000, // $ffe8            ( u1 u2 -- u3 )      quotient
   QUOus = 16'b 1111_1111_1110_1001, // $ffe9            ( u1 n2 -- n3 )      quotient
   QUOsu = 16'b 1111_1111_1110_1010, // $ffea            ( n1 u2 -- n3 )      quotient
   QUOss = 16'b 1111_1111_1110_1011, // $ffeb            ( n1 n2 -- n3 )      quotient             -- default

   IMUL  = 16'b 1111_1111_1111_????,
   MPY   = 16'b 1111_1111_1111_00??,
   MPH   = 16'b 1111_1111_1111_01??,
   MPL   = 16'b 1111_1111_1111_10??,
   MULuu = 16'b 1111_1111_1111_??00,
   MULus = 16'b 1111_1111_1111_??01,
   MULsu = 16'b 1111_1111_1111_??10,
   MULss = 16'b 1111_1111_1111_??11,
   MPYuu = 16'b 1111_1111_1111_0000, // $fff0            ( u1 u2 -- ud )   lower_product upper_product
   MPYus = 16'b 1111_1111_1111_0001, // $fff1            ( u1 n2 -- d  )   lower_product upper_product
   MPYsu = 16'b 1111_1111_1111_0010, // $fff2            ( n1 u2 -- d  )   lower_product upper_product
   MPYss = 16'b 1111_1111_1111_0011, // $fff3            ( n1 n2 -- d  )   lower_product upper_product   -- default
   MPHuu = 16'b 1111_1111_1111_0100, // $fff4            ( u1 u2 -- u3 )   upper_product
   MPHus = 16'b 1111_1111_1111_0101, // $fff5            ( u1 n2 -- n3 )   upper_product
   MPHsu = 16'b 1111_1111_1111_0110, // $fff6            ( n1 u2 -- n3 )   upper_product
   MPHss = 16'b 1111_1111_1111_0111, // $fff7            ( n1 n2 -- n3 )   upper_product                 -- default
   MPLuu = 16'b 1111_1111_1111_1000, // $fff8            ( u1 u2 -- u3 )   lower_product
   MPLus = 16'b 1111_1111_1111_1001, // $fff9            ( u1 n2 -- u3 )   lower_product
   MPLsu = 16'b 1111_1111_1111_1010, // $fffa            ( n1 u2 -- u3 )   lower_product
   MPLss = 16'b 1111_1111_1111_1011; // $fffb            ( n1 n2 -- u3 )   lower_product                 -- default

localparam  [1:0]    // Signedness! -- suffix to Mulitply or Divide
   uu    =  2'b 00,
   us    =  2'b 01,
   su    =  2'b 10,
   ss    =  2'b 11;  // default is signed,signed

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
