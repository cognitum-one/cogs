/* ******************************************************************************************************************************

   Copyright © 2023 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : Glenturret

   Description          : SIMD engine for AI emulation

*********************************************************************************************************************************
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*********************************************************************************************************************************
   NOTES:


*********************************************************************************************************************************

A2_SIMD_CoP #(
   .BASE (13'h00000)
   ) inst (
   // IO Interface
   .imiso      (imiso[32:0]),
   .imosi      (imosi[47:0]),
   .interrupt  (interrupt),
   // WRAM Interface
   .nmosi      (nmosi[271:0]),            // {nreq,nen[3:0],nwr,nadr[9:0],nwrd[255:0]}
   .nmiso      (nmiso[259:0]),            // {nack,nrdd[255:0]}
   // Conditions
   .cond       (AIcond[ 3:0]),             // AI block signaling conditions
   // Global
   .clock      (clock),
   .reset      (reset)
   );

********************************************************************************************************************************/

`ifdef   STANDALONE
   `define  aSIMD_base         13'h1040
   `define  aSIMD_PTR           9'h104
   `define  aSIMD_PTR_0        13'h1040
   `define  aSIMD_PTR_1        13'h1041
   `define  aSIMD_PTR_2        13'h1042
   `define  aSIMD_PTR_3        13'h1043
   `define  aSIMD_PTR_4        13'h1044
   `define  aSIMD_PTR_5        13'h1045
   `define  aSIMD_PTR_6        13'h1046
   `define  aSIMD_PTR_7        13'h1047
   `define  aSIMD_PTR_8        13'h1048
   `define  aSIMD_PTR_9        13'h1049
   `define  aSIMD_PTR_10       13'h104a
   `define  aSIMD_PTR_11       13'h104b
   `define  aSIMD_PTR_12       13'h104c
   `define  aSIMD_PTR_13       13'h104d
   `define  aSIMD_PTR_14       13'h104e
   `define  aSIMD_PTR_15       13'h104f
   `define  aSIMD_MAC          13'h1050
   `define  aSIMD_THD          13'h1051
   `define  aSIMD_ROC          13'h1052
   `define  aSIMD_EDG          13'h1053
   `define  aSIMD_IOP          13'h1054
   `define  aSIMD_IOA          13'h1055
   `define  aSIMD_GBL          13'h1056
   `define  aSIMD_CSR          13'h1057
`else
   `include "A2_project_settings.vh"
`endif

//`ifndef  tCQ
//`define  tCQ   #1
//`endif

module   A2_SIMD_CoP #(
   parameter   [12:0]   BASE  = 13'h00000 // I/O Base Address
   )(
   // IO Interface
   output wire [`IMISO_WIDTH-1:0]   imiso,
   input  wire [`IMOSI_WIDTH-1:0]   imosi,
   output wire                      interrupt,
   // SRAM Interface
   output wire [`SMOSI_WIDTH-1:0]   nmosi,            // {nreq,nen[3:0],nwr,nadr[9:0],nwrd[255:0]}
   input  wire [`SMISO_WIDTH-1:0]   nmiso,            // {nack,nrdd[255:0]}
   input  wire [           255:0]   nmask,            //      nmask[255:0]
   // Conditions
   output wire [             3:0]   cond,             // AI block signaling conditions
   // Global
   output wire                      MPPon,
   input  wire                      clock,
   input  wire                      reset
   );

// Nets -------------------------------------------------------------------------------------------------------------------------

// Physical Registers
reg   [17:5]   PTR   [0:15];                    // 16 Pointer Registers
wire  [31:0]   MIR;                             // Macro Instruction Register
reg   [31:0]   THD;                             // ReLU Threshhold for activation decisions
reg   [31:0]   ROC;                             // Row Column register into SIMD array
reg   [31:0]   EDG;                             // Edge Register at SIMD array
reg   [31:0]   IOA;                             // I/O address to Work RAM for a whole plane
reg   [31:0]   GBL;                             // Global Register input to SIMD array
reg   [31:0]   CSR;                             // Control\Status Register
reg   [31:0]   nEDG;                            // next Edge

reg   [ 8:0]   RC;                              // Repeat Counter
reg            ENABLE;                          // Clock Enable for the entire module

wire           dPTR,dMAC,dTHD,dROC,dEDG,dIOP,dIOA,dGBL,dCSR;
wire           MIRor, MIRir, wMIR, rMIR;        // MIR FIFO controls
wire           IPDor, IPDir, wIPD, rIPD;        // IOP input FIFO controls
wire           OPDor, OPDir, wOPD, rOPD;        // IOP output FIFO controls
wire           wMAC,  rMAC,  wIDP, rIDP,  wDP, rDP, wBS, rBS;
wire           clk,  DPor, DPir;

// Microcode
reg   [255:0]  IR;                              // Instruction Block : 16-Words
wire  [ 31:0]  uC;                              // Microword
reg   [ 15:2]  PC;                              // microcode PC,         16384 x  32 bits (64kBytes) of Work RAM
reg   [ 15:5]  current_uPC_mod8;                // Address of Instruction block in IR modulo 8
reg   [ 12:0]  IPTR,JPTR,KPTR,RPTR,WPTR;        // Pointers to Work RAM : 2048 x 256 bits (64kBytes) of Work RAM

wire  [ 15:5]  jmpadr;
wire  [ 11:2]  br_adr;
wire  [ 10:0]  padr;
wire           branch, next, exit, ldEDG, shGBL;
// imosi
wire           iWR, iRD;
wire           dRPTR, dMACRO, dack;
// Microcode
wire           ld_IPTR,ld_JPTR,ld_KPTR,ld_DPTR; // load controls for pointer registers
wire           inc_IPTR, inc_JPTR, inc_KPTR, inc_RPTR, inc_WPTR;
wire           dec_IPTR, dec_JPTR, dec_KPTR, dec_RPTR, dec_WPTR;
wire           IOPshift8;
// IO plane
wire  [ 31:0]  DPo;           // IO Plane Output
wire  [255:0]  BSo, iBS;      // Broadside In and Out
// WRAM Address selection
wire           dorp;                            // Select data or program for access to Work RAM
wire  [ 15:5]  data_adr,prog_adr,next_adr,incr_adr, decr_adr;
// WRAM nmosi
wire           enWM;
wire           wrWM;
// Reduction pipeline feedback to the SIMD array
wire  [15:0]   redux_16, adjusted_sum;
wire  [ 7:0]   redux_8, redux_8s, redux_8f;
wire  [ 7:0]   bias;
wire           redux_1;

// Collate and Decollate buses ==================================================================================================

// Collate nmosi
reg            nreq;                            // Enable WRAM Access
wire  [  3:0]  nen;                             // Bank Enables
wire           nwr;                             // Write Command
wire  [  9:0]  nadr;                            // Address within banks
wire  [255:0]  nwrd;                            // Write Data

assign   nmosi[271:0] =  {nreq,nen[3:0],nwr,nadr[9:0],nwrd[255:0]};

// Decollate nmiso
wire  [  3:0]  nack;                            // Acknowledge per bank
wire  [255:0]  nrdd;                            // Read data

assign  {nack[3:0], nrdd[255:0]} =  nmiso[259:0];

assign   iM[255:0]   =  nrdd[255:0];

// Decollate imosi
wire  [ 2:0]   cIO;                             // imosi command
wire  [31:0]   iIO;                             // imosi write data
wire  [12:0]   aIO;                             // imose address

assign  {cIO[2:0], iIO[31:0], aIO[12:0]}  =  imosi;

// Collate imiso
wire  [31:0]   IOo;                             // imiso read data
wire           IOk;                             // imiso acknowledge

assign   imiso[32:0] =  {IOk, IOo[31:0]};

// SIMD CONTROL REGISTERS =======================================================================================================
//
//                  3 3 2 2 2 2 2 2 2 2 2 2 1 1 1 1 1 1 1 1 1 1 0 0 0 0 0 0 0 0 0 0
//                  1 0 9 8 7 6 5 4 3 2 1 0 9 8 7 6 5 4 3 2 1 0 9 8 7 6 5 4 3 2 1 0
//                 +---------------------+-----------------+-------+-------+-------+
//         MAC     |     Entry Point     | Repeat (0..511) |   I   |   J   |   K   |
//                 +---------------------+---------+-------+-------+-------+-------+
//         THD     |XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX|           Threshold           |
//                 +-------------------------------+-------------------------------+
//         ROC     |              ROWo             |            COLUMNo            |
//                 +-------------------------------+-------------------------------+
//         EDG     |              iROW           EDGE          iCOLUMN             |
//                 +-------------------------------+-------------------------------+
//         IOP     |                 IO plane Data Register (FIFO)                 |
//                 +-------------------------------+-------------------------------+
//         IOA     |     IO plane Write address    |     IO plane Read address     |
//                 +-------------------------------+-------------------------------+
//         GBL     |                             Global                            |
//                 +-+---------------------------------------------------------+-+-+
//         CSR     |E|                     Control \ Status                    |S|O|
//                 +-+---------------------------------------------------------+-+-+
//                   \                                                           \ \__ SIMDon flag to wRAM
//                    \_ Enable (Clock enable whole block)                        \___ 0: Saturated 8-bit  1: bsfloat
//
//    MACRO call
//    I, J & K select from a pointer array (16)  each 11+2 bits wide. On entry into the macro the selected pointers are loaded
//    to counters that advance per the microcode.  The entries in the pointer array are not changed except by the host writing
//    to the pointer array!  This simplifies bit-serial arithmetic as the pointers in the array can always point to bit-0 and
//    the macro increments through the operands and no return of the pointer is required.
//
//    POINTER ARRAY
//    Each pointer has 2 extra bits
//       bit[11]  A = 0: static      1: auto-inc/dec
//       bit[12]  D = 0: decrement   1: increment
//
//                  3                                     1 1 1
//                  1                                     2 1 0 9 8 7 6 5 4 3 2 1 0
//                 +-------------------------------------+---+---------------------+
//         P15     |XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX|   |       pointer       |
//                 +-------------------------------------+---+---------------------+
//                 :                                     :   :                     :
//                 :                                     :   :                     :
//                 +-------------------------------------+---+---------------------+
//         P1      |XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX|   |       pointer       |
//                 +-------------------------------------+---+---------------------+
//         P0      |XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX|D A|       pointer       |
//                 +-------------------------------------+---+---------------------+
//                                                        \_/ \_____/\_____/\_____/
//                                           ____________________|      |      |____________________
//                                          |                           |                           |
//                           +---+----------v----------+ +---+----------v----------+ +---+----------v----------+
//                           |d a|       I counter     | |d a|      J counter      | |d a|      K counter      |
//                           +---+---------------------+ +---+---------------------+ +---+---------------------+
//
//     example : SIMD (ADD, 12, I, J, K)
//
// Physical Registers ===========================================================================================================

assign   dPTR  = aIO[12:4] == `aSIMD_PTR,             // Pointer Array
         dMAC  = aIO[12:0] == `aSIMD_MAC,             // Macro
         dTHD  = aIO[12:0] == `aSIMD_THD,             // Threshold
         dROC  = aIO[12:0] == `aSIMD_ROC,             // Row\Column
         dEDG  = aIO[12:0] == `aSIMD_EDG,             // Edge of Array
         dIOP  = aIO[12:0] == `aSIMD_IOP,             // IO Plane
         dIOA  = aIO[12:0] == `aSIMD_IOA,             // IO Address
         dGBL  = aIO[12:0] == `aSIMD_GBL,             // Global
         dCSR  = aIO[12:0] == `aSIMD_CSR;             // Control Status

assign   iWR   =  cIO == 3'b110,
         iRD   =  cIO == 3'b101,
         wMAC  =  dMAC & iWR,                         // Write MAC attempts to write to the pipe and will only succeed if ready
         rMAC  =  dMAC & iRD,                         // Read MAC does NOT pop the pipe its just a peek
         wIDP  =  dIOP & iWR,
         rIDP  =  dIOP & iRD;

assign   dack  =  (aIO[12:0] >= `aSIMD_base) && (aIO[12:0] <= `aSIMD_CSR );

assign   MPPon =  CSR[0];

// Registers ====================================================================================================================

always @(posedge clock or posedge reset)
   if      (reset)         CSR <= `tCQ  32'h0;
   else if (iWR && dCSR)   CSR <= `tCQ  iIO[0];

assign   clk = clock | ~CSR[31];

always @(posedge clk) if (iWR && dPTR)  PTR[aIO[3:0]]    <= `tCQ  iIO[17:5];     // Pointer Block (16 registers of 13 bits)

always @(posedge clk) if (iWR && dTHD)  THD              <= `tCQ  iIO[31:0];     // Threshhold Register
always @(posedge clk) if (iWR && dROC)  ROC              <= `tCQ  iIO[31:0];     // Row\Column Register
always @(posedge clk) if (iWR && dEDG)  EDG              <= `tCQ  iIO[31:0];     // Edge Register
                 else if (      ldEDG)  EDG              <= `tCQ  nEDG;
always @(posedge clk) if (iWR && dGBL)  GBL              <= `tCQ  iIO[31:0];     // Global Data Register
                 else if (      shGBL)  GBL              <= `tCQ  GBL >>> 1;     // Shift out

// Macro Register ---------------------------------------------------------------------------------------------------------------
// Holds current and next Macro to be executed.  CC can monitor input ready for the A2S to load the next Macro.
A2_pipe_cg #(32) i_MIR (.Po(MIR[31:0]),.Por(MIRor),.Pir(MIRir), .iP(iIO[31:0]),.wP(wMIR),.rP(next),.clock(clock),.reset(reset));

// IO Plane =====================================================================================================================
// 32x8 array
//     <----------------------------------------------------IOW---------------------------------------------------->
//     <------------redux_8------------>
//     +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---//--+---+---+---+---+---+---+---+---+---+---+  array bits
//     |   |   |   |   |   |   |   |   |   |   |   |   |   |   |   |  //   |   |   |   |   |   |   |   |   |   |   |  [255:224]
//     +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+-//+---+---+---+---+---+---+---+---+---+---+---+
//     |   |   |   |   |   |   |   |   |   |   |   |   |   |   |   |// |   |   |   |   |   |   |   |   |   |   |   |  [223:192]
//     +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---//--+---+---+---+---+---+---+---+---+---+---+---+
//     |   |   |   |   |   |   |   |   |   |   |   |   |   |   |  //   |   |   |   |   |   |   |   |   |   |   |   |  [191:160]
//     +---+---+---+---+---+---+---+---+---+---+---+---+---+---+-//+---+---+---+---+---+---+---+---+---+---+---+---+
//     |   |   |   |   |   |   |   |   |   |   |   |   |   |   |// |   |   |   |   |   |   |   |   |   |   |   |   |  [159:128]
//     +---+---+---+---+---+---+---+---+---+---+---+---+---+---//--+---+---+---+---+---+---+---+---+---+---+---+---+
//     |   |   |   |   |   |   |   |   |   |   |   |   |   |  //   |   |   |   |   |   |   |   |   |   |   |   |   |  [127:096]
//     +---+---+---+---+---+---+---+---+---+---+---+---+---+-//+---+---+---+---+---+---+---+---+---+---+---+---+---+
//     |   |   |   |   |   |   |   |   |   |   |   |   |   |// |   |   |   |   |   |   |   |   |   |   |   |   |   |  [095:064]
//     +---+---+---+---+---+---+---+---+---+---+---+---+---//--+---+---+---+---+---+---+---+---+---+---+---+---+---+
//     |   |   |   |   |   |   |   |   |   |   |   |   |  //   |   |   |   |   |   |   |   |   |   |   |   |   |   |  [063:032]
//     +---+---+---+---+---+---+---+---+---+---+---+---+-//+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//     |   |   |   |   |   |   |   |   |   |   |   |   |// |   |   |   |   |   |   |   |   |   |   |   |   |   |   |  [031:000]
//     +---+---+---+---+---+---+---+---+---+---+---+---//--+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//                                                                                 <--------------bias------------->
//     <----------------------------------------------------IOR---------------------------------------------------->
//
// The IO plane is a 32 x 8 pipe with input and output ports that report status to the A2S over the condition code ports. The
// SIMD State machine loads and stores, to and from the Work RAM and moves data in and out of these ports or ports into
// and out of, the reduction pipeline.

A2_pipeNbs #(32,8) mut (
   // output port
   .Po      (DPo[31:0]),
   .Por     (DPor),
   .rP      (rDP),
   // input port
   .iP      (iIO[31:0]),
   .wP      (wDP),
   .Pir     (DPir),
   // broadside port
   .PBo     (BSo),        // Broadside Output
   .iPB     (iBS),        // Broadside Input
   .wPB     (wBS),        // write Broadside
   .rPB     (rBS),        // read  Broadside
   // global
   .clock   (clock),
   .reset   (reset)
   );

// IO plane operations

assign   iBS[255:0]  =  IOPshift8   ?  {redux_8[7:0], BSo[255:8]} : nrdd[255:0];

assign   cond  = {DPir,DPor,MIRir};                                         // Condition Codes to A2S

assign   bias  =  BSo[7:0];

// imiso outputs ----------------------------------------------------------------------------------------------------------------

assign   IOo   =  {32{iRD && dPTR}} & {16'h0,PTR[aIO[ 3:0]],5'h0}
               |  {32{iRD && dMAC}} &            MIR[31:0]           // 16
               |  {32{iRD && dTHD}} &            THD[31:0]           // 17
               |  {32{iRD && dROC}} &            ROC[31:0]           // 18
               |  {32{iRD && dEDG}} &            EDG[31:0]           // 19
               |  {32{iRD && dIOP}} &            DPo[31:0]           // 20
               |  {32{iRD && dIOA}} &            IOA[31:0]           // 21
               |  {32{iRD && dGBL}} &            GBL[31:0]           // 22
               |  {32{iRD && dCSR}} &            CSR[31:0]           // 23
               ;  // leave it!

assign   IOk   =   cIO[2] & dack;

// SIMD-PE Microword definition =================================================================================================
//
//    +-------+-------+-------+-------+-------+-------+-------+-------+
//    |   F   |   F   |   F   |   F   |   F   |   F   |   F   |   F   |
//    +-------+-------+-------+-------+-------+-------+-------+-------+
//     1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1  default = FFFFFFFF
//     --one-- -hld- hld hld hld hld hld -hld- --nxt-- ------tbd-------
//
//                                              |1 1 1 1 1 1          | \__ Long Jump Address / Macro Entry Point
//                                              |5 4 3 2 1 0 9 8 7 6 5| /
//                                              |    address  bits    |
//                                              | |1 1          |     | \__ Local Jump Address
//                                              | |1 0 9 8 7 6 5|4 3 2| /   No WRAM access if bits 11:5 are the same
//                                              | |             |     |     just move about the 256-bit IR register!
//     3 3 2 2 2 2 2 2 2 2 2 2 1 1 1 1 1 1 1 1 1|1|0 0 0 0 0 0 0|0 0 0|
//     1 0 9 8 7 6 5 4 3 2 1 0 9 8 7 6 5 4 3 2 1|0|9 8 7 6 5 4 3|2 1 0|
//    +-------+-----+---+---+---+---+---+-----+-+---------------------++---------------------------------------------+
//    |  fn   | sA  |sB |sC |sD |sE |sMo| MEM |0|       Jump          || ->  Jump to 256-bit boundary in work memory |
//    +---|---+--|--+-|-+-|-+-|-+-|-+-|-+--|--+---+-------------+-----++---------------------------------------------+
//        |      |    |   |   |   |   |    |  |1 0|     uPC     | nPC || R--> Jump offset to a 32-bit microword (*1) |
//        |      |    |   | +-+-+ |   |    |  +---+-+-----+---+-+-----++---------------------------------------------+
//        |      |    |   | |0 1|-----|------>|1 1 0| typ |wrp| dist  || NEWS repeat, decrementing distance to zero  >--------+
//        |      |    |   | +-+-+ |   |    |  +-----+-----+---+-------++---------------------------------------------+        |
//    +---v---+  |    |   |   |   |   |    |  |1 1 1|  see below (*2) || Bit level controls                          |        |
//    | Truth |  |    |   |   |   |   |    |  +-----+-----------------++---------------------------------------------+        |
//    | table |  |    |   |   |   |   |    |                                                                                  |
//    | of 2  |  |    |   |   |   |   |    |                                                                                  |
//    | binary|  |    |   |   |   |   |    |                                                                                  |
//    | opnds |  |    |   |   |   |   |    |                                                      +--MEM--+-----+             |
//    | e.g.  |  |    |   |   |   |   |    +------------------------------------------------------> 0 0 0 | @I  |             |
//    | L:1100|  |    |   |   |   |   |                                             +--Mo-+---+   | 0 0 1 | @J  |             |
//    | R:1010|  |    |   |   |   |   +---------------------------------------------> 0 0 | A |   | 0 1 0 | @K  |             |
//    | "and" |  |    |   |   |   |                                 +--E--+------+  | 0 1 | B |   | 0 1 1 | @R  |             |
//    |fn=1000|  |    |   |   |   +---------------------------------> 0 0 |  A   |  | 1 0 | C |   | 1 0 0 | !I  |             |
//    +-------+  |    |   |   |                     +--D--+------+  | 0 1 | ~E   |  | 1 1 | D |   | 1 0 1 | !J  |             |
//               |    |   |   +---------------------> 0 0 |  A   |  | 1 0 | iM   |  +-----+---+   | 1 1 0 | !K  |             |
//               |    |   |         +--C--+------+  | 0 1 | iD   |  | 1 1 | hold |                | 1 1 1 | !W  |             |
//               |    |   +---------> 0 0 |  0   |  | 1 0 | iM   |  +-----+------+                +-------+-----+             |
//               | +--B--+------+   | 0 1 | ~C   |  | 1 1 | hold |                                                            |
//               | | 0 0 | A    |   | 1 0 | carry|  +-----+------+                                                            |
//               | | 0 1 | C    |   | 1 1 | hold |                \__'count' changes 'A' to ~D                                |
//               | | 1 0 | carry|\  +-----+------+                 \                                                          |
//               | | 1 1 | hold | \      'count' forces ~C          \_Must be set to iD (NEWS) to use wrp,typ,distance! <-----+                               |   |
//               | +-----+------+  \                                                                 /     /
//               |                  \____'count' changes 'A' to ~B                                  /     /
//               |                                      ___________________________________________      /
//               |                                     /                                                /
//           +---A---+--------+                     +-wrp-+----------------------------+            +--typ--+--------------+
//           | 0 0 0 | A op B |          bits[5:4]--> 1 1 | Wrap: from opposite side   | bits[8:6]--> 0 0 0 | North        |
//           | 0 0 1 | A op C |                     | 1 0 | Edge: repeat current value |            | 0 0 1 | East         |
//           | 0 1 0 | A op D |                     | 0 1 | ROW for N,S  COL for E,W   |            | 0 1 0 | South        |
//           | 0 1 1 | A op E |                     | 0 0 | Zeros:                     |            | 0 1 1 | West         |
//           | 1 0 0 | M op B |                     +-----+----------------------------+          __| 1 0 0 | Global       |
//           | 1 0 1 | M op A |                                                                  /  | 1 0 1 | Row          |
//           | 1 1 0 | M op D |                                              auto shift global__/   | 1 1 0 | Column       |
//           | 1 1 1 |  hold  |                                                                     | 1 1 1 | Row & Column |
//           +-------+--------+                                                                     +-------+--------------+
//
// Notes:   (*1)  Jump taken if repeat value is non-zero else next,  repeat value is decremented
//
//          (*2) : 1 1 0 0 0 0 0 0 0 0 0 0
//                 1 0 9 8 7 6 5 4 3 2 1 0
//                +-+-+-+-+-+-+-+-+-+-+-+-+
//                |1 1 1|N|C|D|P|A|E|G|-|M|
//                +-+-+-+-+-+-+-+-+-+-+-+-+
//                  \|/   \ \ \ \ \ \ \ \ \__0___ 0:              1: Memory Access Enable
//                   &  &  \ \ \ \ \ \ \ \___1___ x:              x:
//                          \ \ \ \ \ \ \____2___ 0:              1: shift Global
//                           \ \ \ \ \ \_____3___ 0: no load      1: load Edge
//                            \ \ \ \ \______4___ 0: load acc     1: accumulate                    === NOT zeroACC
//                             \ \ \ \_______5___ 0: hold ppl     1: enable reduction pipeline     === enPPL
//                              \ \ \________6___ 0: count into D 1: clear D                       === clear count MSB
//                               \ \_________7___ 0: no counting  1: count E with ABCD             === count mode
//                                \__________8___ 0: exit         1: next                          === reload MACRO
//
// Microcode Control ============================================================================================================

// Decollate MACRO Instruction
wire  [10:0]   entry_point    = MIR[31:21];
wire  [ 8:0]   repeat_count   = MIR[20:12];
wire  [ 3:0]   pointer_I      = MIR[11:08],
               pointer_J      = MIR[07:04],
               pointer_K      = MIR[03:00];

localparam  IDLE = 0, FETCH = 1, EXECUTE = 2;

reg   [2:0] FSM;

always @(posedge clk) if (reset)          FSM   <= `tCQ  1'b1 << IDLE;
   else case (1'b1)
      FSM[IDLE]   :   if (MIRor)          FSM   <= `tCQ  1'b1 << FETCH;
      FSM[FETCH]  :                       FSM   <= `tCQ  1'b1 << EXECUTE;
      FSM[EXECUTE]:   if (exit &&  MIRor) FSM   <= `tCQ  1'b1 << FETCH;
                 else if (exit && !MIRor) FSM   <= `tCQ  1'b1 << IDLE;
      endcase

wire  idle     =  FSM[IDLE];
wire  fetch    =  FSM[FETCH];
wire  execute  =  FSM[EXECUTE];
wire  ireq     =  idle    & MIRor
               |  execute & MIRor;

assign   padr  =  ireq  ?  entry_point : PC;

always @(posedge clk) if (fetch)            IR[255:0] <= `tCQ           nrdd[255:0];

always @(posedge clk) if (fetch)             PC[15:2] <= `tCQ  {  entry_point[10:0], 3'b000};
                 else if (execute && next)   PC[15:2] <= `tCQ  PC + 1'b1;

always @(posedge clk) if (fetch)       RC[8:0]  <= `tCQ    repeat_count[ 8:0];

always @(posedge clk) if (fetch)    IPTR[12:0]  <= `tCQ  PTR[pointer_I][12:0];
                 else if (inc_IPTR) IPTR[10:0]  <= `tCQ        incr_adr[10:0];
                 else if (dec_IPTR) IPTR[10:0]  <= `tCQ        decr_adr[10:0];

always @(posedge clk) if (fetch)    JPTR[12:0]  <= `tCQ  PTR[pointer_J][12:0];
                 else if (inc_JPTR) JPTR[10:0]  <= `tCQ        incr_adr[10:0];
                 else if (dec_JPTR) JPTR[10:0]  <= `tCQ        decr_adr[10:0];

always @(posedge clk) if (fetch)    KPTR[12:0]  <= `tCQ  PTR[pointer_K][12:0];
                 else if (inc_KPTR) KPTR[10:0]  <= `tCQ        incr_adr[10:0];
                 else if (dec_KPTR) KPTR[10:0]  <= `tCQ        decr_adr[10:0];

wire  ld_RWPTR =  iWR & dIOA;

always @(posedge clk) if (ld_RWPTR)  IOA[31:0]  <= `tCQ        iIO[31:0];
                 else if (inc_RPTR)  IOA[10:0]  <= `tCQ   incr_adr[10:0];
                 else if (dec_RPTR)  IOA[10:0]  <= `tCQ   decr_adr[10:0];
                 else if (inc_WPTR)  IOA[26:16] <= `tCQ   incr_adr[10:0];
                 else if (dec_WPTR)  IOA[26:16] <= `tCQ   decr_adr[10:0];


assign   uC[31:0] =  IR[255:0] >> (32 * PC[4:2]);

wire     RCzero   =  RC[8:0] == {9{1'b0}};

// Processing Elements ==========================================================================================================

localparam   [1:0]     N = 2'b00, E = 2'b01, S = 2'b10, W = 2'b11;

reg  [255:0]   A,B,C,D,E;

// Microcontrol bits:
wire   [3:0]   fn       =   uC[31:28];
wire   [2:0]   sA       =   uC[27:25];
wire   [1:0]   sB       =   uC[24:23];
wire   [1:0]   sC       =   uC[22:21];
wire   [1:0]   sD       =   uC[20:19];
wire   [1:0]   sE       =   uC[18:17];
wire   [1:0]   sMo      =   uC[16:15];
wire   [2:0]   MEM      =   uC[14:12];
wire           jump     =  ~uC[11];
wire  [10:0]   jadr     =   uC[10:0];
assign         branch   =   uC[11] & ~uC[10];      // Decrement and test repeat value for zero
wire   [9:0]   badr     =   uC[9:0];
wire           news     =   uC[11:9] == 3'b110;
wire           step     =   uC[11:9] == 3'b111;
wire           typ      =   uC[8:6];
wire           wrp      =   uC[5:4];
wire           dist     =   uC[4:0];
assign         exit     =   step & ~uC[8];
assign         next     =   step &  uC[8];
wire           count    =   step &  uC[7];
wire           clrD     =   step &  uC[6];
wire           enPPL    =   step &  uC[5];
wire           enACC    =   step &  uC[4];
assign         ldEDG    =   step &  uC[3];
assign         shGBL    =   step &  uC[2];

assign         inc_IPTR =  (MEM[1:0]==2'b00)  & IPTR[11] &  IPTR[12],
               dec_IPTR =  (MEM[1:0]==2'b00)  & IPTR[11] & ~IPTR[12],
               inc_JPTR =  (MEM[1:0]==2'b01)  & JPTR[11] &  JPTR[12],
               dec_JPTR =  (MEM[1:0]==2'b01)  & JPTR[11] & ~JPTR[12],
               inc_KPTR =  (MEM[1:0]==2'b10)  & KPTR[11] &  KPTR[12],
               dec_KPTR =  (MEM[1:0]==2'b10)  & KPTR[11] & ~KPTR[12],
               inc_RPTR =  (MEM[2:0]==3'b011) & RPTR[11] &  KPTR[12],
               dec_RPTR =  (MEM[2:0]==3'b011) & RPTR[11] & ~KPTR[12],
               inc_WPTR =  (MEM[2:0]==3'b111) & WPTR[11] &  WPTR[12],
               dec_WPTR =  (MEM[2:0]==3'b111) & WPTR[11] & ~WPTR[12];

wire    [2:0]  sADR     =   MEM[2:0]==3'b111 ? 3'b100 : {1'b0,MEM[1:0]};

assign         jmpadr   =   jadr;
assign         br_adr   =   badr;
assign         enWM     =   step & uC[0];
assign         wrWM     =   MEM[2];

wire           si       =   redux_1;      // Shift input

wire [255:0]   iM, iD,  // Inputs from Memory and D-plane
               lhs   = {         iM, A } >> (256 *  sA[2]),
               rhs   = { A, E, D, C, B } >> (256 * {sA[2] & ~sA[0],sA[1:0]}),
               logic = {256{fn[0]}}  & ~lhs & ~rhs
                     | {256{fn[1]}}  & ~lhs &  rhs
                     | {256{fn[2]}}  &  lhs & ~rhs
                     | {256{fn[3]}}  &  lhs &  rhs,
               carry =   A & B  | ~A & C,
               Wd    = { D,C,B,A }  >> (256 * sMo[1:0]),

               c1    =  A & E,
               c2    =  B & c1,
               c3    =  C & c2,
               c4    =  D & c3,

               shC   = {si,C[255:1]},                             // Shift serial-in through C
               nBA   =  count ? ~B :  A,                          // If counting use ~B rather than A
               nDA   =  count ? (wrWM  ? {256{1'b0}} : ~D ) : A,  // Clear D when writing during count else use ~D, rather than A
               ldA   =  count ? E      : {256{~&sA[2:0]}},
               ldB   =  count ? c1     : {256{~&sB[1:0]}},
               ldC   =  count ? c2     : {256{~&sC[1:0]}},
               ldD   =  count ? c3     : {256{~&sD[1:0]}},
               ldE   =                   {256{~&sE[1:0]}},
               Mo    =  count ? c4     :  Wd,
               Eo    =  E     |          {256{count}},
               Do    =  D;

genvar i;
for (i=0; i<256; i=i+1) begin : FF
   always @(posedge clk) if (ldA[i]) A[i] <= `tCQ   logic[i];                          // Primary operand
   always @(posedge clk) if (ldB[i]) B[i] <= `tCQ  {carry[i],  C[i], nBA[i]} >> sB;    // Secondary operand
   always @(posedge clk) if (ldC[i]) C[i] <= `tCQ  {carry[i], ~C[i],   1'b0} >> sC;
   always @(posedge clk) if (ldE[i]) E[i] <= `tCQ  {   iM[i], ~E[i],   A[i]} >> sE;    // Enable  (Activity)
   always @(posedge clk) if (ldD[i]) D[i] <= `tCQ  {   iM[i], iD[i], nDA[i]} >> sD;
   end

// D-plane formats ==============================================================================================================
//
// 16x16 array
//               +--- rows -->
//               |
//             +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+  array bits
//      +------|0,0|0,1|0,2|0,3|0,4|0,5|0,6|0,7|0,8|0,9|0,a|0,b|0,c|0,d|0,e|0,f|  [255:240]
//      |      +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//      |      |1,0|1,1|1,2|1,3|1,4|1,5|1,6|   |   |   |   |   |   |   |   |1,f|  [239:224]
//      c      +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//      o      |2,0|2,1|2,2|2,3|2,4|2,5|   |   |   |   |   |   |   |   |   |2,f|  [223:208]
//      l      +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//      u      |3,0|3,1|3,2|3,3|3,4|   |   |   |   |   |   |   |   |   |   |3,f|  [207:192]
//      m      +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//      n      |4,0|4,1|4,2|4,3|4,4|   |   |   |   |   |   |   |   |   |   |4,f|  [191:176]
//             +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//      m      |5,0|5,1|5,2|   |   |5,5|   |   |   |   |   |   |   |   |   |5,f|  [175:160]
//      a      +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//      j      |6,0|6,1|   |   |   |   |6,6|   |   |   |   |   |   |   |   |6,f|  [159:144]
//      o      +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//      r      |7,0|   |   |   |   |   |   |7,7|   |   |   |   |   |   |   |f,f|  [143:128]
//      |      +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//      V      |8,0|   |   |   |   |   |   |   |8,8|   |   |   |   |   |   |8,f|  [127:112]
//             +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//             |9,0|   |   |   |   |   |   |   |   |9,9|   |   |   |   |   |9,f|  [111:096]
//             +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//             |a,0|   |   |   |   |   |   |   |   |   |a,a|   |   |   |   |a,f|  [095:080]
//             +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//             |b,0|   |   |   |   |   |   |   |   |   |   |b,b|   |   |   |b,f|  [079:064]
//             +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//             |c,0|   |   |   |   |   |   |   |   |   |   |   |c,c|   |   |c,f|  [063:048]
//             +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//             |d,0|   |   |   |   |   |   |   |   |   |   |   |   |d,d|   |d,f|  [047:032]
//             +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//             |e,0|   |   |   |   |   |   |   |   |   |   |   |   |   |e,e|e,f|  [031:016]
//             +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//             |f,0|   |   |   |   |   |   |   |   |   |   |   |   |   |f,e|f,f|  [015:000]
//             +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
//
// NEWS Network =================================================================================================================

localparam   SIZE  =  256;
localparam   ROWS  =   16;
localparam   COLS  =   16;
localparam   North = 0, South = 1, East = 2, West = 3, Global = 4, Row = 5, Column = 6, Intersect = 7;

reg  [SIZE-1:0]   iD;      // Shift Out
reg  [COLS-1:0]   Po;      // Plane Out

integer r, c;

// Recollate inputs into a 2D array
reg  plane    [15:0][15:0];
reg  shift    [15:0][15:0];
reg  [255:0]   iDN, iDF;

always @*
   for (r=0; r<ROWS; r=r+1)          // rows
      for (c=0; c<COLS; c=c+1)       // columns
         plane[r][c] = Do[COLS * (ROWS-1-r) + COLS -1 - c];

// 2D shifting with edge modes
always @* begin
   nEDG  = 32'h0;
   for (r=0; r<ROWS; r=r+1)   // rows
      for (c=0; c<COLS; c=c+1) begin  // columns
         case (typ)                       // ones   edge              opposite                     shift
         North    : shift[r][c] = r== ROWS-1 ? ({1'b1, plane[ROWS-1][c], plane[0][c]     } >> ~wrp) : plane[r-1][c]; // ->N
         South    : shift[r][c] = r== 0      ? ({1'b1, plane[0]     [c], plane[ROWS-1][c]} >> ~wrp) : plane[r+1][c]; // ->S
         East     : shift[r][c] = c== 0      ? ({1'b1, plane[r]    [00], plane[r][COLS-1]} >> ~wrp) : plane[r][c-1]; // ->E
         West     : shift[r][c] = c== COLS-1 ? ({1'b1, plane[r][COLS-1], plane[r][00]    } >> ~wrp) : plane[r][c+1]; // ->W
         Global   : shift[r][c] = GBL[0];
         Row      : shift[r][c] = ROC[c];
         Column   : shift[r][c] = ROC[ROWS+r];
         Intersect: shift[r][c] = ROC[ROWS+r] & ROC[c];
         endcase
         nEDG[c]        = nEDG  & ROC[ROWS+r] & ROC[c]      | plane[r][c];
         nEDG[ROWS+r]   = nEDG  & ROC[r]      & ROC[COLS+c] | plane[r][c];
         end
   end

// Recollate back to a single vector input to D plane
always @*
   for (r=0; r<ROWS; r=r+1)       // rows
      for (c=0; c<COLS; c=c+1)   // columns
         iDN[(ROWS-r-1) * ROWS + (COLS-c-1)] = shift[r][c];

// Memory Access ----------------------------------------------------------------------------------------------------------------

//                     WPTR       RPTR
assign   data_adr  =  {IOA[26:16],IOA[10:0], KPTR, JPTR, IPTR}    >> (11 * sADR[2:0]),
         next_adr  =   dorp ? data_adr : padr,
         incr_adr  =   data_adr + 1,
         decr_adr  =   data_adr - 1;

// Work RAM Interface -----------------------------------------------------------------------------------------------------------

// Work Memory is 64 kBytes in 8 banks
// Each bank is 512 deep and 128 bits wide
// For this application (Salsa is different!) there are two different access modes.  In normal execution the banks are accesses
// to provide bit serial data to the processing elements.  In I/O mode the incoming or outgoing data is 'corner-turned' so that
// byte/word parallel data is seen in the I/O.
// For execute mode the accesses are to a bit-plane (256 bit-0s of 256 bit-serial words,for example) So the address lsb is to a
// bit plane.
//

always @(posedge clk) nreq <= `tCQ CSR[31];         // Take over ownership of the Work RAM

assign   nwrd[255:0] =  Mo[255:0],
        nmask[255:0] =   E[255:0],
         nadr[  8:0] =  next_adr[15:7],                     // Address within bank
         nen [  3:0] =  enWM << next_adr[6:5],              // Choose a bank to enable
         nwr         =  wrWM;                               // Write Enable

// Reduction Pipeline ===========================================================================================================
// Start by masking off inactive components and counting in twos!
//    This version inverts the msb of xs to simplify summation of signed integers as per:
//       Agrawal & Rao. "On Multiple Operand Addition of Signed Binary Numbers". 1978. Used in Baugh-Wooley multiplier!
//    This makes all higher bits zero, so no sign extension is required and the overflow happens out of range!
//    This effectively adds 4 to every value. So for a sum of 128 2-bit values we subtract 128 x 4 at the end (well, add -512)!

reg   [ 2:0]   xs [0:127];
reg   [15:0]   SUM64k, SUMTB;
reg   [ 2:0]   eSUM;
reg            zSUM;

wire     enable_sum  =  next &  uC[0];
wire     reload_acc  =  next &  uC[1];
wire     result_typ  =  next &  uC[2];
assign   IOPshift8   =  next &  uC[1];

// Control Pipeline from microcode
always @(posedge clk) if (enPPL || |eSUM[1:0]) eSUM   <= `tCQ {eSUM[0],enable_sum};
always @(posedge clk) if (enPPL ||  eSUM[  0]) zSUM   <= `tCQ          enACC;

integer j;
always @* for (j=0; j<128; j=j+1) begin : XS2s
   if (enable_sum)
      case ({E[2*j+1 +: 2], A[2*j+1 +: 2]})
         4'b11_00                               :  xs[j] = 3'b010;      // -2 => 2 excess '0's
         4'b01_00, 4'b01_10, 4'b10_00, 4'b10_01 :  xs[j] = 3'b011;      // -1 => 1 excess '0'
         default                                   xs[j] = 3'b100;      //  0 => 0 equal number of '0's and '1's
         4'b01_01, 4'b01_11, 4'b10_10, 4'b10_11 :  xs[j] = 3'b101;      //  1 => 1 excess '1'
         4'b11_11                               :  xs[j] = 3'b110;      //  2 => 2 excess '1's
         endcase
   else  xs[j] = 3'b100;
   end

// Now sum them
reg   [9:0] xsum;
always @* begin
   xsum = 10'b10_0000_0000;       // load -512, compensate for lack of sign extension
   for (i=0; i<128; i=i+1) begin
      xsum = xsum + xs[i][2:0] ;
      end
   end

// So now we have the number of excess 1's or 0's that we can accumulate over multiple 256 bit batches
// Sum of up tp 64k Binary Products
// Then on a completed batch (can be just 1 cycle) add in Threshold and Bias values
always @(posedge clk) if (enPPL &&  eSUM[0]) SUM64k   <= `tCQ  ( {16{~enACC}}  &  $signed(SUM64k))  +  $signed(xsum);

assign   adjusted_sum   =  $signed(SUM64k) + $signed(THD) + xsfloat2int(bias);

// Where & how to store results! ------------------------------------------------------------------------------------------------
// For ReLU take the sign bit and shift it into the C register, then save the C register every 256 'cycles'
// For 8-bit results select either a saturated 8-bit integer or a xsfloat representation and shift it into the IO plane
// and save every 32 'cycles' to Work RAM

assign redux_16 = adjusted_sum[15:0];            // Sum with Threshold and Bias
// Save an 8-bit saturable integer
//    10000000 saturated negative
//    01111111 saturated positive
//    else     -127..+126
assign redux_8s = adjusted_sum  <= -128 ? 8'b10000000
                : adjusted_sum  >=  127 ? 8'b01111111
                : adjusted_sum[7:0];
// Save an 8-bit 'float' to the D-plane
assign redux_8f = int2xsfloat(adjusted_sum[15:0]);
// Using RELU for activation the result is simply the sign bit
assign redux_1  = adjusted_sum[15];

// Send results back to the SIMD Array
assign redux_8  = result_typ ? redux_8f : redux_8s;

// FUNCTIONS ====================================================================================================================

function  [7:0] int2xsfloat;
input    [15:0] int16;
   begin
   reg   [14:0] magnitude;
   magnitude   = (( {15{int16[15]}} ^ int16[14:0] ) + int16[15]);     // 2's complement

   casez(magnitude[14:0])                    //  sign    exponent  mantissa
      15'b 1??_????_????_????: int2xsfloat = { int16[15], 4'b1111, magnitude[13:11] };
      15'b 01?_????_????_????: int2xsfloat = { int16[15], 4'b1110, magnitude[12:10] };
      15'b 001_????_????_????: int2xsfloat = { int16[15], 4'b1101, magnitude[11:9]  };
      15'b 000_1???_????_????: int2xsfloat = { int16[15], 4'b1100, magnitude[10:8]  };
      15'b 000_01??_????_????: int2xsfloat = { int16[15], 4'b1011, magnitude[ 9:7]  };
      15'b 000_001?_????_????: int2xsfloat = { int16[15], 4'b1010, magnitude[ 8:6]  };
      15'b 000_0001_????_????: int2xsfloat = { int16[15], 4'b1001, magnitude[ 7:5]  };
      15'b 000_0000_1???_????: int2xsfloat = { int16[15], 4'b1000, magnitude[ 6:4]  };
      15'b 000_0000_01??_????: int2xsfloat = { int16[15], 3'b011,  magnitude[ 5:2]  };
      15'b 000_0000_001?_????: int2xsfloat = { int16[15], 3'b010,  magnitude[ 4:1]  };
      15'b 000_0000_0001_????: int2xsfloat = { int16[15], 3'b001,  magnitude[ 3:0]  };
      15'b 000_0000_0000_????: int2xsfloat = { int16[15], 3'b000,  magnitude[ 3:0]  };    // denormal -- No implied leading "1"
      endcase
   end
endfunction

function [15:0] xsfloat2int;
input    [ 7:0] xsfloat;
input           half;            // Add half a step
   begin
   reg   [14:0] bias, twoscomp;
   casez(xsfloat[6:3])
      4'b 1111: bias = {  1'b1, xsfloat[2:0], half, 9'b000000000 };
      4'b 1110: bias = {  2'b01, xsfloat[2:0], half, 8'b00000000 };
      4'b 1101: bias = {  3'b001, xsfloat[2:0], half, 7'b0000000 };
      4'b 1100: bias = {  4'b0001, xsfloat[2:0], half, 6'b000000 };
      4'b 1011: bias = {  5'b00001, xsfloat[2:0], half, 5'b00000 };
      4'b 1010: bias = {  6'b000001, xsfloat[2:0], half, 4'b0000 };
      4'b 1001: bias = {  7'b0000001, xsfloat[2:0], half, 3'b000 };
      4'b 1000: bias = {  8'b00000001, xsfloat[2:0], half, 2'b00 };
      4'b 011?: bias = {  9'b000000001, xsfloat[3:0], half, 1'b0 };
      4'b 010?: bias = { 10'b0000000001, xsfloat[3:0], half      };
      4'b 001?: bias = { 11'b00000000001, xsfloat[3:0]           };
      4'b 000?: bias = { 11'b00000000000,  xsfloat[3:0]          };    // denormal -- No implied leading "1"
      endcase

   twoscomp    = ({15{xsfloat[7]}} ^ bias) + 1'b1;
   xsfloat2int = {xsfloat[7],twoscomp[14:0]};
   end
endfunction

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef         verify_AI_CoP          // Add this as a Simulator command line define
`define        STANDALONE
module t;

parameter      HALFCYCLE = 10;

wire  [32:0]   imiso;
reg   [47:0]   imosi;
wire           interrupt;
// SRAM Interface
wire [271:0]   nmosi;            // {nreq,nen[3:0],nwr,nadr[9:0],nwrd[255:0]}
reg  [259:0]   nmiso;            // {nack,nrdd[255:0]}
// Conditions
wire [  3:0]   cond;             // AI block signaling conditions
// Global
reg            clk;
reg            reset;

A2_AI_CoP #(0)(
   .imiso     (imiso),
   .imosi     (imosi),
   .interrupt (interrupt),
   .nmosi     (nmosi),           // {nreq,nen[3:0],nwr,nadr[9:0],nwrd[255:0]}
   .nmiso     (nmiso),           // {nack,nrdd[255:0]}
   .cond      (cond),            // AI block signaling conditions
   .clk     (clk),
   .reset     (reset)
   );

initial begin
   clk = 1'b1;
   forever begin
      #HALFCYCLE clk = ~clk;
      end
   end

initial begin
   reset =  1'b0;
   imosi =  48'b0;
   nmiso =  260'b0;
   repeat (100) @(posedge clk);
   $finish;
   end

endmodule
`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
