/*==============================================================================================================================

   Copyright © 2023 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : A2S v2r3
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
                        //          M[a-addr]   memory location at the aligned address
                        //          cellsize    the size, in address units(bytes), of a cell. 4 for a 32-bit machine
                        //          A,B & C are registers, R is the return stack
   PUTA  = 6'b 00_0000, //  0  !a      ( x -- )(A: a-addr)   (M: M[a-addr] <- x)
   PUTB  = 6'b 00_0001, //  1  !b      ( x -- )(B: a-addr)   (M: M[a-addr] <- x)
   PUTC  = 6'b 00_0010, //  2  !c      ( x -- )(C: a-addr)   (M: M[a-addr] <- x)
   PUT   = 6'b 00_0011, //  3  !       ( x a-addr -- )       (M: M[a-addr] <- x)  -- double pop!!
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

   DECA  = 6'b 01_0000, // 16  a-      ( -- )  (A: a-addr -- (a-addr - cellsize)
   DECB  = 6'b 01_0001, // 17  b-      ( -- )  (B: a-addr -- (a-addr - cellsize)
   DECC  = 6'b 01_0010, // 18  c-      ( -- )  (C: a-addr -- (a-addr - cellsize)
   SNB   = 6'b 01_0011, // 19  snb     ( skip to next bundle )
   res2  = 6'b 01_0100, // 21  ---     reserved
   res3  = 6'b 01_0101, // 22  ---     reserved
   res4  = 6'b 01_0110, // 23  ---     reserved
   NOT   = 6'b 01_0111, // 21  ~       ( x -- ~x)
   T2A   = 6'b 01_1000, // 24  >a      ( x -- )(A: -- x)
   T2B   = 6'b 01_1001, // 25  >b      ( x -- )(B: -- x)
   T2C   = 6'b 01_1010, // 26  >c      ( x -- )(C: -- x)
   T2R   = 6'b 01_1011, // 27  >r      ( x -- )(R: -- x)
   A2T   = 6'b 01_1100, // 28  a>      ( -- x )(A: x -- x)
   B2T   = 6'b 01_1101, // 29  b>      ( -- x )(B: x -- x)
   C2T   = 6'b 01_1110, // 30  c>      ( -- x )(C: x -- x)
   R2T   = 6'b 01_1111, // 31  r>      ( -- x )(R: x --  )

   NIP   = 6'b 10_0000, // 32  nip     ( x1 x2 -- x2 )
   DROP  = 6'b 10_0001, // 33  drop    ( x1 x2 -- x1 )
   DUP   = 6'b 10_0010, // 34  dup     ( x -- x x  )
   OVER  = 6'b 10_0011, // 35  over    ( x1 x2 -- x1 x2 x1  )
   SWAP  = 6'b 10_0100, // 36  ><      ( x1 x2 -- x2 x1      )
   ROT3  = 6'b 10_0101, // 37  ><<     ( x1 x2 x3 -- x2 x3 x1 )
   ROT4  = 6'b 10_0110, // 38  ><<<    ( x1 x2 x3 x4 -- x2 x3 x4 x1)
   SEL   = 6'b 01_0100, // 20  ?:      ( x1 x2 flag  -- x3 = flag == FALSE ? x2 : x1 )
   ADD   = 6'b 10_1000, // 40  +       ( n1|u1 n2|u2 -- n3|u3 = n1|u1 + n2|u2 )
   SUB   = 6'b 10_1001, // 41  -       ( n1|u1 n2|u2 -- n3|u3 = n1|u1 - n2|u2 )
   SLT   = 6'b 10_1010, // 42  <       ( n1 n2 -- flag = (n1  < n2) ? TRUE : FALSE )
   ULT   = 6'b 10_1011, // 43  u<      ( u1 u2 -- flag = (u1 u< u2) ? TRUE : FALSE )
   EQ    = 6'b 10_1100, // 44  =       ( x1 x2 -- flag = (x1 == x2) ? TRUE : FALSE )
   XOR   = 6'b 10_1101, // 45  ^       ( x1 x2 -- x3 = bit-wise XOR of x1, x2 )
   IOR   = 6'b 10_1110, // 46  |       ( x1 x2 -- x3 = bit-wise IOR of x1, x2 )
   AND   = 6'b 10_1111, // 47  &       ( x1 x2 -- x3 = bit-wise AND of x1, x2 )

   ZERO  = 6'b 11_0000, // 48  0       ( -- 0 )
   ONE   = 6'b 11_0001, // 49  1       ( -- 1 )
   CO    = 6'b 11_0010, // 50  co      ( -- )(R:rtn-addr -- P+(*1))  (P: u -- rtn_addr) *1: P is advanced to next 2 byte boundary
   RTN   = 6'b 11_0011, // 51  rtn     ( -- )(R:rtn-addr -- )        (P: u -- rtn-addr)
   NOP   = 6'b 11_0100, // 52  nop     ( -- )
   PFX   = 6'b 11_0101, // 53  prefix  ( -- )                        (P: u1 -- u2 = u1 + 2)
   LIT   = 6'b 11_0110, // 54  value   ( -- x )                      (P: u1 -- u2 = u1 + 2)
   EXT   = 6'b 11_0111, // 55  EXT     ( variable ) see extension function coding
   res5  = 6'b 11_1000, // 56  ---     reserved
   CALL  = 6'b 11_1001, // 57  name    ( -- )(R: -- P+(*1))          (P: u1 -- u2 = u1 + offset)   *1
   JN    = 6'b 11_1010, // 58  n->     ( n -- )                      (P: u1 -- u2 = u1 + (n<0  ? offset : 2))
   JZ    = 6'b 11_1011, // 59  0->     ( x -- )                      (P: u1 -- u2 = u1 + (x==0 ? offset : 2))
   JANZ  = 6'b 11_1100, // 60  a-->    ( -- )(A: u -- u-1 )          (P: u1 -- u2 = u1 + (A<>0 ? offset : 2))
   JBNZ  = 6'b 11_1101, // 61  b-->    ( -- )(B: u -- u-1 )          (P: u1 -- u2 = u1 + (B<>0 ? offset : 2))
   JCNZ  = 6'b 11_1110, // 62  c-->    ( -- )(C: u -- u-1 )          (P: u1 -- u2 = u1 + (C<>0 ? offset : 2))
   JMP   = 6'b 11_1111; // 63  ->      ( -- )                        (P: u1 -- u2 = u1 + offset)

localparam  [1:0] // 2-bit Order Encoding ---------------------------------------------------------------------------------------
   MT    = 2'b 00,      // 50  Empty 2-bit order:  NEVER EXECUTED!!
   CAL2  = 2'b 01,      // 55  name    ( -- )  (R: -- P+(*1))        (P: u1 -- u2 = u1 + offset)   *1
   LIT2  = 2'b 10,      // 54  value   ( -- x )                      (P: u1 -- u2 = u1 + 2)
   RTN2  = 2'b 11;      // 51  rtn     ( -- )  (R: rtn-addr -- )     (P: u1 -- rtn-addr)

// 16-bit Function Encoding =====================================================================================================
// We are adding Relative extensions from the bottom up and immediates from the top down

localparam  [15:0]

// I/O accesses -----------------------------------------------------------------------------------------------------------------

   IO    = 16'b 0???_????_????_????, //    4 I/O command codes & 13-bit I/O address

// pop and push requirements are coded into the assignments
//              00                     p0p1
//              010                    p1p0
//              011                    p1p1
//                                    pop   push
//                                     \   /
   IORC  = 16'b 000?_????_????_????, // p0p1 $0000 + io-addr  ( -- x )    (IO: x  <- IO[io-addr] <- 0            )  Read and Clear
   IORD  = 16'b 001?_????_????_????, // p0p1 $2000 + io-addr  ( -- x )    (IO: x  <- IO[io-addr]                 )  Read
   IOWR  = 16'b 010?_????_????_????, // p1p0 $4000 + io-addr  ( x -- )    (IO:       IO[io-addr] <- x            )  Write
   IOSW  = 16'b 011?_????_????_????, // p1p1 $6000 + io-addr  ( x1 -- x2 )(IO: x2 <- IO[io-addr] <- x1 atomically)  Swap

// Shift Immediate Operators ----------------------------------------------------------------------------------------------------

   SHIM  = 16'b 1000_0???_????_????, //   8 codes 5 used

   RORI  = 16'b 1000_0000_????_????, // p1p1 $8000 + imm8  ( u1|n1 -- u2|n2 = u1|n1 <>> imm8 )  Rotate  Shift Right Immediate
   ROLI  = 16'b 1000_0001_????_????, // p1p1 $8100 + imm8  ( u1|n1 -- u2|n2 = u1|n1 <<> imm8 )  Rotate  Shift Left  Immediate
   LSRI  = 16'b 1000_0010_????_????, // p1p1 $8200 + imm8  ( n1    --  n2   = n1    >>  imm8 )  Logical Shift Right Immediate
   LSLI  = 16'b 1000_0011_????_????, // p1p1 $8300 + imm8  ( n1    --  n2   = n1    <<  imm8 )  Logical Shift Leftt Immediate
   ASRI  = 16'b 1000_0100_????_????, // p1p1 $8300 + imm8  ( n1    --  n2   = n1    >>> imm8 )  Arithmetic Shift Right Immediate

// Condition Code extension------------------------------------------------------------------------------------------------------
//
   COCO  = 16'b 1000_1???_????_????, //   8 codes 1 used

   CC    = 16'b 1000_1000_????_????, // p0p1 $8800 +imm8   ( -- flag ) if Condition-Code[imm8] flag = TRUE else flag = FALSE

// ==============================================================================================================================
// EXTENSIONS THAT DO NOT USE AN IMMEDIATE
// ==============================================================================================================================
//
// The following Extensions all have the first hex character as 0xf, So there is a total of 4096 codes
//

// Spill and Fill, R and D Stacks -----------------------------------------------------------------------------------------------

   SPILR = 16'b 1111_1011_0110_0000, //                    ( Spill from R-stack to Data Memory)
   SPILD = 16'b 1111_1011_0110_0001, //                    ( Spill from D-stack to Data Memory)
   FILLR = 16'b 1111_1011_0110_0010, //                    ( Fill  from Data Memory to R-stack)
   FILLD = 16'b 1111_1011_0110_0011, //                    ( Fill  from Data Memory to D-stack)

// Population Count of XOR ------------------------------------------------------------------------------------------------------
   POPS  = 16'b 1111_1011_0111_00??, // any if the next four

   POPC  = 16'b 1111_1011_0111_0000, // p1p1 $fb70         ( n1 n2 -- n3 = number-of-bits-set-in        (n1 & ~n2) )
   EXCS  = 16'b 1111_1011_0111_0001, // p1p1 $fb71         ( n1 n2 -- n3 = excess number-of-bits-set-in (n1 & ~n2) )
   CLZ   = 16'b 1111_1011_0111_0010, // p1p1 $fb72         ( n1 n2 -- n3 = number-of-leading-zeros-in   (n1 & ~n2) )
   CTZ   = 16'b 1111_1011_0111_0011, // p1p1 $fb73         ( n1 n2 -- n3 = number-of-trailing-zeros-in  (n1 & ~n2) )

// Swap Bytes extension

   BSWP  = 16'b 1111_1011_0111_0111, // p1p1 $fbc1         ( x1 -- x2 = {x1[7:0],x1[15:8],x1[23:16],x1[31:24]}

// Shift Relative Operators -----------------------------------------------------------------------------------------------------

   SHRL  = 16'b 1111_1011_0111_1???, //  8 codes   5 used

   ROR   = 16'b 1111_1011_0111_1000, // p2p1 $c000         ( x1 x2 -- x3 =  x1 <>> x2 ) Rotate  Shift    Right Relative
   ROL   = 16'b 1111_1011_0111_1001, // p2p1 $c100         ( x1 x2 -- x3 =  x1 <<> x2 ) Rotate  Shift    Left  Relative
   LSR   = 16'b 1111_1011_0111_1010, // p2p1 $c200         ( x1 x2 -- x3 =  x1 >>  x2 ) Logical Shift    Right Relative
   LSL   = 16'b 1111_1011_0111_1011, // p2p1 $c300         ( x1 x2 -- x3 =  x1 <<  x2 ) Logical Shift    Left Relative
   ASR   = 16'b 1111_1011_0111_1100, // p2p1 $c300         ( x1 x2 -- x3 =  x1 >>> x2 ) Arithmetic Shift Right Relative

// Multiply ---------------------------------------------------------------------------------------------------------------------

   MPYS  = 16'b 1111_1011_10??_????, // 64 codes 12 used
                                     //                         top                          top
   MPYuu = 16'b 1111_1011_1000_0000, // p2p2 $fb80         ( u1 u2 -- ud )     lower_product upper_product
   MPYus = 16'b 1111_1011_1000_0001, // p2p2 $fb81         ( u1 n2 -- d  )     lower_product upper_product
   MPYsu = 16'b 1111_1011_1000_0010, // p2p2 $fb82         ( n1 u2 -- d  )     lower_product upper_product
   MPYss = 16'b 1111_1011_1000_0011, // p2p2 $fb83         ( n1 n2 -- d  )     lower_product upper_product   -- default

   MPHuu = 16'b 1111_1011_1000_0100, // p2p1 $fb84         ( u1 u2 -- u3 )     upper_product
   MPHus = 16'b 1111_1011_1000_0101, // p2p1 $fb85         ( u1 n2 -- n3 )     upper_product
   MPHsu = 16'b 1111_1011_1000_0110, // p2p1 $fb86         ( n1 u2 -- n3 )     upper_product
   MPHss = 16'b 1111_1011_1000_0111, // p2p1 $fb87         ( n1 n2 -- n3 )     upper_product                 -- default

   MPLuu = 16'b 1111_1011_1000_1000, // p2p1 $fb88         ( u1 u2 -- u3 )     lower_product
   MPLus = 16'b 1111_1011_1000_1001, // p2p1 $fb89         ( u1 n2 -- u3 )     lower_product
   MPLsu = 16'b 1111_1011_1000_1010, // p2p1 $fb8a         ( n1 u2 -- u3 )     lower_product
   MPLss = 16'b 1111_1011_1000_1011, // p2p1 $fb8b         ( n1 n2 -- u3 )     lower_product                 -- default

   // Groupings
   MULuu = 16'b 1111_1011_1000_??00,
   MULus = 16'b 1111_1011_1000_??01,
   MULsu = 16'b 1111_1011_1000_??10,
   MULss = 16'b 1111_1011_1000_??11,
   MPY   = 16'b 1111_1011_1000_00??,
   MPH   = 16'b 1111_1011_1000_01??,
   MPL   = 16'b 1111_1011_1000_10??,

// Divide -----------------------------------------------------------------------------------------------------------------------

   DIVS  = 16'b 1111_1011_11??_????, // 64 codes 12 used
                                     //                         top                          top
   DIVuu = 16'b 1111_1011_1100_0000, // p2p2 $fbc0         ( u1 u2 -- u3 u4 )  remainder     quotient
   DIVus = 16'b 1111_1011_1100_0001, // p2p2 $fbc1         ( u1 n2 -- n3 n4 )  remainder     quotient
   DIVsu = 16'b 1111_1011_1100_0010, // p2p2 $fbc2         ( n1 u2 -- n3 n4 )  remainder     quotient
   DIVss = 16'b 1111_1011_1100_0011, // p2p2 $fbc3         ( n1 n2 -- n3 n4 )  remainder     quotient        -- default

   MODuu = 16'b 1111_1011_1100_0100, // p2p1 $fbc4         ( u1 u2 -- u3 )     remainder
   MODus = 16'b 1111_1011_1100_0101, // p2p1 $fbc5         ( u1 n2 -- n3 )     remainder
   MODsu = 16'b 1111_1011_1100_0110, // p2p1 $fbc6         ( n1 u2 -- n3 )     remainder
   MODss = 16'b 1111_1011_1100_0111, // p2p1 $fbc7         ( n1 n2 -- n3 )     remainder                     -- default

   QUOuu = 16'b 1111_1011_1100_1000, // p2p1 $fbc8         ( u1 u2 -- u3 )     quotient
   QUOus = 16'b 1111_1011_1100_1001, // p2p1 $fbc9         ( u1 n2 -- n3 )     quotient
   QUOsu = 16'b 1111_1011_1100_1010, // p2p1 $fbca         ( n1 u2 -- n3 )     quotient
   QUOss = 16'b 1111_1011_1100_1011, // p2p1 $fbcb         ( n1 n2 -- n3 )     quotient                      -- default

   // Groupings
   IDIV  = 16'b 1111_1011_1110_????,   // Any Integer Divide
   DIV   = 16'b 1111_1011_1110_00??,
   REM   = 16'b 1111_1011_1110_01??,
   QUO   = 16'b 1111_1011_1110_10??,

// Floating Point ===============================================================================================================

   HALF  = 16'b 1111_1100_????_????, //  256 Half precision codes -- reserved
   SNGL  = 16'b 1111_1101_????_????, //  256 Single precision codes
   DUBL  = 16'b 1111_1110_????_????, //  256 Double precision codes
   XTND  = 16'b 1111_1111_????_????, //  256 eXtended precision codes -- reserved

// Single Precision -------------------------------------------------------------------------------------------------------------

   FMAD  = 16'b 1111_1101_0000_0???, // p3p1 $fd00 +rm     ( f1 f2 f3 -- f4 = rm(f3 *  f2  + f1) )
   FMSB  = 16'b 1111_1101_0000_1???, // p3p1 $fd08 +rm     ( f1 f2 f3 -- f4 = rm(f3 *  f2  - f1) )
   FMNA  = 16'b 1111_1101_0001_0???, // p3p1 $fd10 +rm     ( f1 f2 f3 -- f4 = rm(f3 * -f2  + f1) )
   FMNS  = 16'b 1111_1101_0001_1???, // p3p1 $fd18 +rm     ( f1 f2 f3 -- f4 = rm(f3 * -f2  - f1) )

   FMUL  = 16'b 1111_1101_0010_0???, // p2p1 $fd20 +rm     ( f1 f2 -- f3 = rm(f2 *  f1      ) )
   FADD  = 16'b 1111_1101_0010_1???, // p2p1 $fd28 +rm     ( f1 f2 -- f3 = rm(f2 *  1.0 + f1) )
   FSUB  = 16'b 1111_1101_0011_0???, // p2p1 $fd30 +rm     ( f1 f2 -- f3 = rm(f2 * -1.0 + f1) )
   FRSB  = 16'b 1111_1101_0011_1???, // p2p1 $fd38 +rm     ( f1 f2 -- f3 = rm(f2 *  1.0 - f1) )

   FDIV  = 16'b 1111_1101_0100_0???, // p2p1 $fd40 +rm     ( f1 f2 -- f3 = rm(f1 / f2) )
   FREM  = 16'b 1111_1101_0100_1???, // p2p1 $fd48 +rm     ( f1 f2 -- f3 = rm(f1 - (f2*n)) ) where n is nearest to f1/f2
   FMOD  = 16'b 1111_1101_0101_0???, // p2p1 $fd50 +rm     ( f1 f2 -- f3 = rm(f1 - (f2*n)) ) where n is closest & not > f1/f2
   FSQR  = 16'b 1111_1101_0101_1???, // p1p1 $fda0 +rm     ( f1    -- f2 = rm(square-root(f)) )

   FF2S  = 16'b 1111_1101_0110_0???, // p1p1 $fd80 +rm     ( f1    -- n1  = rm( sfix(f)) )    # float to signed
   FS2F  = 16'b 1111_1101_0110_1???, // p1p1 $fd88 +rm     ( n1    -- f1  = rm(float(n)) )    # signed to float
   FF2U  = 16'b 1111_1101_0111_0???, // p1p1 $fd90 +rm     ( f1    -- u1  = rm( ufix(f)) )    # float to unsigned
   FU2F  = 16'b 1111_1101_0111_1???, // p1p1 $fd98 +rm     ( u1    -- f1  = rm(float(u)) )    # unsigned to float

   spr17 = 16'b 1111_1101_1000_0000, // 1 spare code
   FCLT  = 16'b 1111_1101_1000_0001, // p2p1 $fd81         ( f1 f2 -- flag ) if  f1 <  f2  then flag = TRUE else flag = FALSE
   FCEQ  = 16'b 1111_1101_1000_0010, // p2p1 $fd82         ( f1 f2 -- flag ) if  f1 =  f2  then flag = TRUE else flag = FALSE
   FCLE  = 16'b 1111_1101_1000_0011, // p2p1 $fd83         ( f1 f2 -- flag ) if  f1 <= f2  then flag = TRUE else flag = FALSE
   FCGT  = 16'b 1111_1101_1000_0100, // p2p1 $fd84         ( f1 f2 -- flag ) if  f1 >  f2  then flag = TRUE else flag = FALSE
   FCNE  = 16'b 1111_1101_1000_0101, // p2p1 $fd85         ( f1 f2 -- flag ) if  f1 <> f2  then flag = TRUE else flag = FALSE
   FCGE  = 16'b 1111_1101_1000_0110, // p2p1 $fd86         ( f1 f2 -- flag ) if  f1 <= f2  then flag = TRUE else flag = FALSE
   spr18 = 16'b 1111_1101_1000_0111, // 1 spare code
   FMAX  = 16'b 1111_1101_1000_1000, // p2p1 $fd88         ( f1 f2 -- f3 = (f1 > f2  ? f1  : f2) )
   FMIN  = 16'b 1111_1101_1000_1001, // p2p1 $fd89         ( f1 f2 -- f3 = (f1 > f2  ? f2  : f1) )
   FSAT  = 16'b 1111_1101_1000_1010, // p2p1 $fd8a         ( f1 f2 -- f3 = (if f1 < 0.0 then 0.0 else min(f1,f2)) )
   spr19 = 16'b 1111_1101_1000_1011, // 1 spare code
   spr20 = 16'b 1111_1101_1000_11??, // 4 spare codes

   FNAN  = 16'b 1111_1101_1000_1100, // p1p1 $fd8c         ( f -- (if f is NaN  then 0.0  else f) )
   FABS  = 16'b 1111_1101_1000_1101, // p1p1 $fd8d         ( f -- |f| )
   FCHS  = 16'b 1111_1101_1000_1110, // p1p1 $fd8e         ( f -- -f  ) simply invert msb!
   spr21 = 16'b 1111_1101_1001_????, // 16 spare codes
   spr22 = 16'b 1111_1101_101?_????, // 32 spare codes
   spr23 = 16'b 1111_1101_11??_????, // 64 spare codes

   // Hardware Groupings
   FM_u  = 16'b 1111_1101_00??_????, // FPU Multiply add unit
   FD_u  = 16'b 1111_1101_01??_????, // FPU Divide unit
   FV_u  = 16'b 1111_1101_011?_????, // FPU conVersion unit  ( 0=f2s, 1=s2f, 2=f2u, 3=u2f)
   FC_u  = 16'b 1111_1101_1000_0???, // FPU Compare unit     ( EQ,GE,LT,LE,GT,NE only)
   FX_u  = 16'b 1111_1101_1000_10??, // FPU Extras unit
   FU_u  = 16'b 1111_1101_1000_11??, // FPU Unary Op unit

// Double Precision -------------------------------------------------------------------------------------------------------------

   DMAD  = 16'b 1111_1110_0000_0???, // p3p1 $fe00 +rm     ( d1 d2 d3 -- d4 = rm(d3 *  d2  + d1) )
   DMSB  = 16'b 1111_1110_0000_1???, // p3p1 $fe08 +rm     ( d1 d2 d3 -- d4 = rm(d3 *  d2  - d1) )
   DMNA  = 16'b 1111_1110_0001_0???, // p3p1 $fe10 +rm     ( d1 d2 d3 -- d4 = rm(d3 * -d2  + d1) )
   DMNS  = 16'b 1111_1110_0001_1???, // p3p1 $fe18 +rm     ( d1 d2 d3 -- d4 = rm(d3 * -d2  - d1) )

   DMUL  = 16'b 1111_1110_0010_0???, // p2p1 $fe20 +rm     ( d1 d2 -- d3 = rm(d2 *  d1      ) )
   DADD  = 16'b 1111_1110_0010_1???, // p2p1 $fe28 +rm     ( d1 d2 -- d3 = rm(d2 *  1.0 + d1) )
   DSUB  = 16'b 1111_1110_0011_0???, // p2p1 $fe30 +rm     ( d1 d2 -- d3 = rm(d2 * -1.0 + d1) )
   DRSB  = 16'b 1111_1110_0011_1???, // p2p1 $fe38 +rm     ( d1 d2 -- d3 = rm(d2 *  1.0 - d1) )

   DDIV  = 16'b 1111_1110_0100_0???, // p2p1 $fe40 +rm     ( d1 d2 -- d3 = rm(d1 / d2) )
   DREM  = 16'b 1111_1110_0100_1???, // p2p1 $fe48 +rm     ( d1 d2 -- d3 = rm(d1 - (d2*n)) ) where n is nearest to d1/d2
   DMOD  = 16'b 1111_1110_0101_0???, // p2p1 $fe50 +rm     ( d1 d2 -- d3 = rm(d1 - (d2*n)) ) where n is closest & not > d1/d2
   DSQR  = 16'b 1111_1110_0101_1???, // p1p1 $fea0 +rm     ( d1    -- d2 = rm(square-root(d1)) )

   DF2S  = 16'b 1111_1110_0110_0???, // p1p1 $fe80 +rm     ( d1    -- n1  = rm( sfix(f)) )    # float to signed
   DS2F  = 16'b 1111_1110_0110_1???, // p1p1 $fe88 +rm     ( n1    -- d1  = rm(float(n)) )    # signed to float
   DF2U  = 16'b 1111_1110_0111_0???, // p1p1 $fe90 +rm     ( d1    -- u1  = rm( ufix(f)) )    # float to unsigned
   DU2F  = 16'b 1111_1110_0111_1???, // p1p1 $fe98 +rm     ( u1    -- d1  = rm(float(u)) )    # unsigned to float

   spr24 = 16'b 1111_1110_1000_0000, // 1 spare code
   DCLT  = 16'b 1111_1110_1000_0001, // p2p1 $fe81         ( d1 d2 -- flag ) if  d1 <  d2  then flag = TRUE else flag = FALSE
   DCEQ  = 16'b 1111_1110_1000_0010, // p2p1 $fe82         ( d1 d2 -- flag ) if  d1 =  d2  then flag = TRUE else flag = FALSE
   DCLE  = 16'b 1111_1110_1000_0011, // p2p1 $fe83         ( d1 d2 -- flag ) if  d1 <= d2  then flag = TRUE else flag = FALSE
   DCGT  = 16'b 1111_1110_1000_0100, // p2p1 $fe84         ( d1 d2 -- flag ) if  d1 >  d2  then flag = TRUE else flag = FALSE
   DCNE  = 16'b 1111_1110_1000_0101, // p2p1 $fe85         ( d1 d2 -- flag ) if  d1 <> d2  then flag = TRUE else flag = FALSE
   DCGE  = 16'b 1111_1110_1000_0110, // p2p1 $fe86         ( d1 d2 -- flag ) if  d1 <= d2  then flag = TRUE else flag = FALSE
   spr25 = 16'b 1111_1110_1000_0111, // 1 spare code
   DMAX  = 16'b 1111_1110_1000_1000, // p2p1 $fe88         ( d1 d2      -- d3 = ( d1  >   d2   ? d1  : d2        ))
   DMIN  = 16'b 1111_1110_1000_1001, // p2p1 $fe89         ( d1 d2      -- d3 = ( d1  >   d2   ? d2  : d1        ))
   DSAT  = 16'b 1111_1110_1000_1010, // p2p1 $fe8a         ( d1 d2      -- d3 = ( d1  <  0.0   ? 0.0 : min(d1,d2)))
   DSEL  = 16'b 1111_1110_1000_1011, // p3p1 $fe8b         ( d1 d2 flag -- d3 = (flag == FALSE ? d2  : d1        ))
   spr27 = 16'b 1111_1110_1000_1100, // 4 spare codes

   DNAN  = 16'b 1111_1110_1000_1100, // p1p1 $fe8c         ( d -- (if d is NaN  then 0.0  else d) )
   DABS  = 16'b 1111_1110_1000_1101, // p1p1 $fe8d         ( d -- |d| )
   DCHS  = 16'b 1111_1110_1000_1110, // p1p1 $fe8e         ( d -- -d  ) simply invert msb!
   DF2F  = 16'b 1111_1110_1001_0???, // p2p1 $fe90 +rm     ( d -- f   ) round  double to single
   FF2D  = 16'b 1111_1110_1001_1???, // p1p2 $fe98 +rm     ( f -- d   ) expand single to double
   spr30 = 16'b 1111_1110_101?_????, // 32 spare codes
   spr31 = 16'b 1111_1110_11??_????; // 64 spare codes

// Suffixes ---------------------------------------------------------------------------------------------------------------------

localparam  [1:0]    // Signedness! -- suffix to Multiply or Divide
   uu    =  2'b 00,
   us    =  2'b 01,
   su    =  2'b 10,
   ss    =  2'b 11;  // default

localparam  [1:0]  // IO OPCODES
   IORCop   =  2'b00,
   IORDop   =  2'b01,
   IOWRop   =  2'b10,
   IOSWop   =  2'b11;

localparam [2:0]  // Rounding Modes
   NEAR  =  3'b 000,
   UP    =  3'b 001,
   DOWN  =  3'b 010,
   TRNC  =  3'b 011,
// MAX   =  3'b 100,
   DFLT  =  3'b 111; // default


////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
