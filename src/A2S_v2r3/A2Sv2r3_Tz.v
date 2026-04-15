/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//
//   Copyright © 2022..2025 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         :  A2S Stack Machine -- variant 2 revision 3
//
//   Description          :  32-bit Processor Core of Zero Address ISA for a pure Stack operation
//
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//
//   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//
//   Variant 2:  6-bit Orders, 32/64-bit only
//
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

//`include "A2_defines.vh"
//`include "A2Sv2r3_Defines.vh"
//`include "A2Sv2r3_Registers.vh"

module A2Sv2r3_TZ #(
   parameter   NCCI  = 17,                         // Number of Condition Codes //JLPL - 11_4_25 - Add 2 JTAG Condition Codes in  //JLPL - 11_8_25 - Add 2 Parity Error Condition Codes In
   parameter   WA2S  = `A2S_MACHINE_WIDTH,         // A2S Machine Width
   parameter   WINS  = `A2S_INSTRUCTION_WIDTH,     // A2S Instruction Word Width
   parameter   WIMM  = `A2S_IMMEDIATE_WIDTH,       // A2S Width of Immediates
   parameter   DRSK  = `A2S_RTRN_STACK_DEPTH,      // A2S Depth of Return Stack
   parameter   DDSK  = `A2S_DATA_STACK_DEPTH,      // A2S Depth of Data Stack
   parameter   WFNC  = `A2S_FUNCTION_WIDTH
   )(
   output wire [`DMOSI_WIDTH-1:0]   dmosi,         // Data Memory  Master-out slave-in
   input  wire [`DMISO_WIDTH-1:0]   dmiso,         // Data Memory  Master-in slave-out

   output wire [        WA2S  :0]   cmosi,         // Code Memory  Master-out slave-in 33 {eCM, aCM}
   input  wire [        WA2S+1:0]   cmiso,         // Code Memory  Master-in slave-out 34 {CMg, CMk, CMo}

   output wire [`IMOSI_WIDTH-1:0]   imosi,         // I/O Register Master-out slave-in
   input  wire [`IMISO_WIDTH-1:0]   imiso,         // I/O Register Master-in slave-out

   input  wire [NCCI          :1]   ccode,         // bit zero is reserved for OR of all others -- fast 'any' detection
   input  wire                      intpt,         // External Interrupt.  Internal ones are called 'events'

   // Global
   output wire                      activ,         // Processor is active
   input  wire                      reset,         // Reset
   input  wire                      fstck,         // Fast Clock
   input  wire                      clock          // Clock
   );

localparam             nLA2S  =  `LG2(WA2S);


localparam              LA2S  =  `LG2(WA2S);
localparam              LRSK  =  `LG2(DRSK);
localparam              LDSK  =  `LG2(DDSK);
localparam [LA2S-1:0]   LZRO  =  {LA2S{1'b0}};
localparam              WOPC  =  `A2S_ORDER_WIDTH;       // Width of Order (Opcode)
localparam              WIOA  =  `A2S_IO_ADDRESS_WIDTH;  // Width of IO Address
localparam [WA2S-1:0]   NBPW  =   WA2S/8;                // Number of Bytes per Word
localparam [WA2S-1:0]   IBPW  =  -NBPW;                  // Inverse NBPW
localparam [WA2S-1:0]   IMM0  =  {WA2S{1'b0}};
localparam [WA2S-1:0]   IMM1  = {{WA2S-1{1'b0}}, 1'b1};
localparam [WA2S-1:0]   IMM2  = {{WA2S-2{1'b0}}, 2'b10};
localparam [WA2S-1:0]   IMM4  = {{WA2S-3{1'b0}}, 3'b100};
localparam [WA2S-1:0]   IMM6  = {{WA2S-3{1'b0}}, 3'b110};
localparam [WA2S-1:0]   FALSE =  {WA2S{1'b0}};
localparam [WA2S-1:0]   TRUE  =  ~FALSE;
localparam [WA2S-1:0]   NEG1  =  ~FALSE;
localparam [WA2S-1:0]   X     =  {WA2S{1'bx}};
localparam [     0:0]   l     =   1'b0,   h = 1'b1,    x = 1'bx;
localparam [     1:0]   ll    =   2'b00, hh = 2'b11,  xx = 2'bxx;
localparam [     2:0]   xxx   =   3'bxxx;
localparam [     3:0]   xxxx  =   4'bxxxx;

`ifdef synthesis
`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2Sv2r3_ISA_TZ.vh"
`else
`include "A2Sv2r3_ISA_TZ.vh" //JLPL - 10_23_25
`endif


// Registers ====================================================================================================================

reg   [WA2S-1:0]  T0,T1,T2,T3;                     // T0..3                   Tstack Registers
reg   [WIMM-1:0]  Y0,Y1,Y2,Y3;                     // Immediate Queue         Y0 is lowest(first)
reg   [WA2S-1:0]  A, B, C;                         // A, B & C                Address Registers
reg   [WA2S-1:0]  O, P;                            // O,P                     Program  Registers  O (advance) P (current)
reg   [WINS+1:0]  I;                               // I                       Instruction register

reg   [     3:0]  FSM;                             // Main State Machine
reg   [     4:0]  YFSM;                            // YQ   State Machine
reg   [     3:0]  Tdepth;                          // Depth of tStack State Machine

reg               Az,Bz,Cz;                        // Zero flag bits for A,B & C
reg               CMseqread;                       // Flag bit Code Mempory   sequential   read in progress
reg               DMreading;                       // Flag bit Data Mempory read  in progress
reg               DMwriting;                       // Flag bit Data Mempory write in progress
reg               Prefix;                          // Flag bit Prefix caught

// Locals =======================================================================================================================
// Memory Inputs
wire  [WA2S-1:0]  CMo, DMo, IOo;                   // Code Memory, Data Memory and I/O Input data
wire              CMk, DMk, IOk;                   // Code Memory, Data Memory and I/O Input acKnowledges
wire              CMg, DMg;                        // Code Memory and Data Memory access request Grant
// Memory Access Control
wire  [     3:0]  k;                               // Pointer for Data In to Tstack
// Processor Stall
wire              stallIO, stall, run;
wire              ckeO,ckeP,ckeDM,ckeCMs,ckeY,ckeI,ckePfx,ckeABC,ckeT,ckeTd,ckeDstk,ckeRstk;
//wire              ckeCMj,ckeFSM;
// Program Counter et al.
wire  [WA2S-1:0]  destination, jumpto, nO, incO, advP, nP, aCM;
wire              seqreq,   eCM;
// Y3..0 -- Instruction / Immediate Queue
wire  [WIMM-1:0]  I0, I1;
wire  [WINS  :0]  YQo;
wire  [WA2S-1:0]  imm, offset;
wire  [WFNC-1:0]  fn;
wire              jit1, jit2, jit3, jit, w2YQ, fi1YQ, fi3YQ, r0YQ, r1YQ, r2YQ, r3YQ, r4YQ, YQmt;
// IQ  Instruction Queue
wire  [WINS+1:0]  incoming, ifromYQ;
wire  [WINS+1:0]  nI, sequent;
wire  [WOPC-1:0]  order, xpslot1, xpslot3, xpslot5;
wire              format1, format1a, format1b, format1c, format3a, format3b, format3c, format3d;
wire              last1, last2, last3, last, IQmt;
// Straight Decodes
wire              eDM_order,    rDM_order,     wDM_order,     xDM_order,
                  lit_order, T0addr_order, jmp2RSo_order, overlap_order, prefix_order, eFN_order;
// FSM & Conditionals
reg   [     1:0]  nFSM;    localparam [1:0] START = 0,     VECTOR= 1,      FETCH = 2,     NEXT = 3;      // FSM States
reg   [     1:0]  snP;     localparam [1:0] sP0   = 0,     sP2   = 1,      sP4   = 2,     sP6  = 3;      // PC advance
reg   [     1:0]  cR, cD;  localparam [1:0] HOLD  = 2'b00, RPLC  = 2'b01,  POP   = 2'b10, PUSH = 2'b11;  // Stack Commands

reg               rIQ,  jump, ldP,  rYQ,  ex,   ldF,  wIQ;
wire              fail, kodd, kevn, lodd, levn, lont, lent;
wire              start, fetch, vector, next;
// Address Generation
wire  [     6:0]  asel;
wire  [WA2S-1:0]  addr;
wire              ABCz;
wire  [     2:0]  sABC;
reg   [WA2S-1:0]  nABC;
reg               ldA, ldB, ldC;
// Execute Datapath
reg   [WA2S-1:0]  nT0;
reg   [     3:0]  wT, Tm;
reg               pop2, sPush;
wire  [     3:0]  ldT;
wire              sub;
// Tstack controls
reg   [     3:0]  nTdepth;                            // Next Depth of tStack State Machine
reg   [     2:0]  s;
reg               X2;                                 // Special case for Exchange XCHG underlapped second cycle
reg               wTd, Ts;
wire              M, N, Q;
// Main Stacks
wire  [WA2S-1:0]  iD, DSo, iR, RSo;                   // Stack in and out data
wire  [LDSK  :0]  Ddepth;
wire  [LRSK  :0]  Rdepth;
wire              Dmt,Dfull,Rmt,Rfull;                // flags
// Outputs to Data Memory
wire  [WA2S-1:0]  aDM, iDM;                           // Address and Write Data
wire              eDM, rDM, wDM;                      // Enable and Write Enable
// Input/Output
wire              eIO;
wire  [     1:0]  cIO;
wire  [WA2S-1:0]  iIO;
wire  [    12:0]  aIO;
wire  [WA2S-1:0]  irdd;
wire              iack;

// Extensions
reg   [WA2S-1:0]  nF0, nF1, nF2;
reg   [     3:0]  wX,  Xm;

//-------------------------------------------------------------------------------------------------------------------------------
// Status Register
reg   [WA2S-1:0]  ST;
wire  [WA2S-1:0]  IOo_intpt, entry_point;
wire              IOk_intpt;
reg               Intpt;

//-------------------------------------------------------------------------------------------------------------------------------
wire  FMstall  = 1'b0;
//wire  IMDstall = 1'b0; //JLPL - 10_27_25
wire  IMDstall; //JLPL - 10_27_25
//-------------------------------------------------------------------------------------------------------------------------------
wire  spilD = 1'b0, spilR = 1'b0, fillD = 1'b0, fillR = 1'b0;
wire  Dstall, Rstall;
//-------------------------------------------------------------------------------------------------------------------------------

// Debug
reg   [     7:0]  caseY, caseD, caseT, caseI, caseF;  // For debug only

// Processor Stall ==============================================================================================================
assign   stallIO  =  IMDstall
`ifdef   A2S_ENABLE_INTERRUPTS
                  |  `ST_WFI                    // Status Register -- Wait for Interrupt
`endif
                  |  jump      & ~CMg           // Jump    but no grant
                  |  eDM_order & ~DMg           // Request but no grant
                  |  DMreading & ~overlap_order // Instruction must wait for incoming data
                  |  DMreading & ~DMk           // Reading but no acknowledge
                  |  DMwriting & ~DMk           // Writing but no acknowledge
                  |  k[3]                       // Pattern is full: Tstack has no more room for mem. data
                  |  Ts                         // Tstack has run dry so stall to reload from main stack
//                |  Dstall
//                |  Rstall
                  ;  // leave the semicolon!
assign   stall    =  stallIO
                  |  eIO & ~iack;

assign   run      =  start | vector | fetch | next & ~stall;
// Clock Enables
assign   ckeCMs   =  1'b1,
//         ckeCMj   =  run |  reset,
         ckeO     = ~reset & (jump | seqreq),
         ckeP     =  run & (jump | ldP ),
         ckeY     =  run,
         ckeI     =  run,
//         ckeFSM   =  run,
         ckePfx   =  run,
         ckeABC   =  run &  ex,
         ckeT     =  run &  ex  |  X2,
         ckeTd    =         ex  & ~FMstall & ~(DMreading & ~overlap_order)  & ~(DMreading & ~DMk) & ~(DMwriting & ~DMk),
         ckeDstk  =  run &  ex  & ~FMstall & ~(DMreading & ~overlap_order)  |  Ts,
         ckeRstk  =  run &  ex,
         ckeDM    =  run &  ex;

// Decollate Bus Inputs =========================================================================================================
//         33   32   31:0
assign   { CMg, CMk, CMo } = cmiso[`DMISO_WIDTH-1:0];    // grant, acknowledge, read data
assign   { DMg, DMk, DMo } = dmiso[`DMISO_WIDTH-1:0];    // grant, acknowledge, read data
assign   {      IOk, IOo } = imiso[`IMISO_WIDTH-1:0];    //        acknowledge, read data

// Data Memory Access Control ===================================================================================================
// DMreading and DMwriting are set when a request is granted by the memory system. They are held on until a memory acknowledge
// resets them.  Instruction flow is stalled if a request is asserted but not granted.  Execution continues until a non-overlap
// instruction is encountered or the Tstack becomes fully assigned by the DMstage logic.

always @(posedge clock)
   if       (reset)  DMwriting <= `tCQ   1'b0;
   else if  (ckeDM)  DMwriting <= `tCQ   next & wDM_order &  DMg;
   else              DMwriting <= `tCQ   DMwriting & ~DMk;

always @(posedge clock)
   if       (reset)  DMreading <= `tCQ   1'b0;
   else if  (ckeDM)  DMreading <= `tCQ   rDM_order &  DMg;
   else              DMreading <= `tCQ   DMreading & ~DMk;

assign   k[0]   =    DMreading & DMk & ~(rDM_order | lit_order),
         k[1]   =    DMreading & DMk &  (rDM_order | lit_order),
         k[2]   =    1'b0,
         k[3]   =    1'b0;

// Advanced Program Counter (O) & Program Counter (P) ===========================================================================
//
// Following a jump we first fetch the target instruction, load it into the I register and follow it immediately with a second
// fetch to the next location and place this in YQ.  This data has one of 3 possible formats.  Based on its inspection we can
// determine that, if the number of immediates called for is zero, then we just fetched a format1 bundle. If the number of
// immediates called for is one we have an immediate and a format3 bundle.  If the number of immediates called for is two then
// we load 2 immediates.  If the number of immediates called for is more than two we keep fetching.
//
// As the cycle that creates the jump flushes the I register and YQ we can safely do these CMreadings and know there is space
// for the returning instruction and immediate words.  We keep sequential fetching as often as we can.
//
// Sequential fetches need to detect that YQ will have room for 2 shorts in the next cycle

localparam [2:0] MPT = 0, UNO = 1, DOS = 2, TRS = 3, QTR = 4;           // Y state machine

assign   seqreq            =  YFSM[MPT]                                                // Sequential Request: queue is empty
                           |  YFSM[UNO] &  ~CMseqread                                  // next YFSM will be 2,1 or 0
                           |  YFSM[DOS] &  ~CMseqread                                  // next YFSM will be 2,1 or 0
                           |  YFSM[DOS] &   CMseqread & (r2YQ | r3YQ) & run            // next YFSM will be   2 or 1
                           |  YFSM[TRS] &  ~CMseqread & (r1YQ | r2YQ  | r3YQ) & run;   // next YFSM will be 2,1 or 0

assign   destination       =  P + offset,                                              // Renamed immediate from YQ

         jumpto            =  vector         ? {CMo[WA2S-1:1],        1'b0 }           // Force an even boundary
                           :  start || Intpt ? {entry_point[WA2S-1:2],2'b00}           // Force an even boundary
                           :  jmp2RSo_order  ?  RSo
                           :                    destination,

         incO[WA2S-1:0]    = {O[WA2S-1:2],2'b00} + 4,                                  // Advance instruction fetch

         advP[WA2S-1:0]    =  P + ({IMM6, IMM4, IMM2, IMM0} >> {snP[1:0],5'b00000}),   // Also used for return to address!!

           nO[WA2S-1:0]    =  jump  ?  jumpto[WA2S-1:0] : incO[WA2S-1:0],              // next Address to Code Memory

           nP[WA2S-1:0]    =  jump  &&  Intpt &&  jmp2RSo_order   ?  RSo               // Interrupt and RTN or CO
                           :  jump  &&  Intpt                     ?  destination       // Interrupt and jump
                           :  jump                                ?  jumpto            // Jump
                           :                                         advP;             // next

always @(posedge clock) if (ckeO  ) O[WA2S-1:0] <= `tCQ  nO;
always @(posedge clock) if (ckeP  ) P[WA2S-1:0] <= `tCQ  nP;
always @(posedge clock) if (ckeCMs) CMseqread   <= `tCQ ~reset & (seqreq & ~jump & CMg | CMseqread & ~CMk);

// Y3..0 -- Immediate Queue  ====================================================================================================
// REMEMBER THESE ARE BIG ENDIAN 16-BIT SHORTS!
// rIQ : from decode case table -- advance instruction queue; if 'last' reload  -- ONLY USED TO LOAD 'I'
// rYQ : from decode case table -- current instruction uses a literal
assign   w2YQ  =  CMseqread & CMk;                                     // Incoming Sequential reads; always for 2 shorts!

assign   jit1  =  last   &  w2YQ   &   YFSM[MPT] & ~rYQ,               // Incoming Just-In-Time
         jit2  =  last   &  w2YQ   &   YFSM[UNO] &  rYQ & ~Prefix,
         jit3  =  last   &  w2YQ   &   YFSM[DOS] &  rYQ &  Prefix,
         jit   =  jit1   |  jit2   |   jit3;

assign   fi1YQ =  last   &  rYQ    &   P[1]                             // Odd  and  1 literal -> Fetch format-1 from YQ
               |  last   & ~rYQ    &  ~P[1],                            // Even and no literal -> Fetch format-1 from YQ
         fi3YQ =  last   &  rYQ    &  ~P[1]                             // Even and  1 literal -> Fetch format-3 from YQ
               |  last   & ~rYQ    &   P[1];                            // Odd  and no literal -> Fetch format-3 from YQ

assign   r0YQ  = ~rYQ    &           ~fi3YQ      & ~fi1YQ,              // These clearly overlap but left for sheer sanity
         r1YQ  =  rYQ    & ~Prefix & ~fi3YQ      & ~fi1YQ               // 1 literal only
               | ~rYQ    &            fi3YQ      & ~fi1YQ,              // 1 format 3 only
         r2YQ  =  rYQ    &  Prefix & ~fi3YQ      & ~fi1YQ
               |  rYQ    & ~Prefix &  fi3YQ      & ~fi1YQ
               | ~rYQ    &           ~fi3YQ      &  fi1YQ,
         r3YQ  =  rYQ    &  Prefix &  fi3YQ      & ~fi1YQ               // 1 imm, 1 pfx, 1 fmt3
               |  rYQ    & ~Prefix & ~fi3YQ      &  fi1YQ,              // 1 imm         1 fmt1
         r4YQ  =  rYQ    &  Prefix & ~fi3YQ      &  fi1YQ;

assign   I0    = CMo[2*WIMM-1:WIMM],                                    // Decollate incoming instruction word
         I1    = CMo[  WIMM-1:0   ];

`define YFSM_MPT  default
`define nYFSM(x) YFSM[4:0] <= `tCQ 5'd1 << (x)

always @(posedge clock) begin
   if (reset || jump)                                  `nYFSM(MPT);  // jump flushes the Queue
   else case (1'b1)                                                  // Nota Bene: Big-endian loads from memory!
      YFSM[UNO]: if (!w2YQ &&  r1YQ  &&  ckeY ) begin  `nYFSM(MPT);                                       caseY <= 00; end
            else if ( w2YQ &&  r1YQ  &&  ckeY ) begin  `nYFSM(DOS);  {      Y1,Y0} <= `tCQ {      I1,I0}; caseY <= 01; end
            else if ( w2YQ &&  r0YQ  &&  ckeY ) begin  `nYFSM(TRS);  {   Y2,Y1   } <= `tCQ {   I1,I0   }; caseY <= 02; end
            else if ( w2YQ &&           !ckeY ) begin  `nYFSM(TRS);  {   Y2,Y1   } <= `tCQ {   I1,I0   }; caseY <= 03; end
            else if ( jit            &&  ckeY ) begin  `nYFSM(MPT);                                       caseY <= 04; end
      YFSM[DOS]: if (!w2YQ &&  r2YQ  &&  ckeY ) begin  `nYFSM(MPT);                                       caseY <= 05; end
            else if (!w2YQ &&  r1YQ  &&  ckeY ) begin  `nYFSM(UNO);  {         Y0} <= `tCQ {         Y1}; caseY <= 06; end
            else if ( w2YQ &&  r4YQ  &&  ckeY ) begin  `nYFSM(MPT);                                       caseY <= 07; end
            else if ( w2YQ &&  r2YQ  &&  ckeY ) begin  `nYFSM(DOS);  {      Y1,Y0} <= `tCQ {      I1,I0}; caseY <= 08; end
            else if ( w2YQ &&  r1YQ  &&  ckeY ) begin  `nYFSM(TRS);  {   Y2,Y1,Y0} <= `tCQ {   I1,I0,Y1}; caseY <= 09; end
            else if ( w2YQ                    ) begin  `nYFSM(QTR);  {Y3,Y2      } <= `tCQ {I1,I0      }; caseY <= 11; end
      YFSM[TRS]: if (!w2YQ &&  r3YQ  &&  ckeY ) begin  `nYFSM(MPT);                                       caseY <= 12; end
            else if (!w2YQ &&  r2YQ  &&  ckeY ) begin  `nYFSM(UNO);  {         Y0} <= `tCQ {         Y2}; caseY <= 13; end
            else if (!w2YQ &&  r1YQ  &&  ckeY ) begin  `nYFSM(DOS);  {      Y1,Y0} <= `tCQ {      Y2,Y1}; caseY <= 14; end
            else if ( w2YQ &&  r3YQ  &&  ckeY ) begin  `nYFSM(DOS);  {      Y1,Y0} <= `tCQ {      I1,I0}; caseY <= 15; end
            else if ( w2YQ &&  r2YQ  &&  ckeY ) begin  `nYFSM(TRS);  {   Y2,Y1,Y0} <= `tCQ {   I1,I0,Y2}; caseY <= 16; end
            else if ( w2YQ &&  r1YQ  &&  ckeY ) begin  `nYFSM(QTR);  {Y3,Y2,Y1,Y0} <= `tCQ {I1,I0,Y2,Y1}; caseY <= 17; end
      YFSM[QTR]: if (          r4YQ  &&  ckeY ) begin  `nYFSM(MPT);                                       caseY <= 18; end
            else if (          r3YQ  &&  ckeY ) begin  `nYFSM(UNO);  {         Y0} <= `tCQ {         Y3}; caseY <= 19; end
            else if (          r2YQ  &&  ckeY ) begin  `nYFSM(DOS);  {      Y1,Y0} <= `tCQ {      Y3,Y2}; caseY <= 20; end
            else if (          r1YQ  &&  ckeY ) begin  `nYFSM(TRS);  {   Y2,Y1,Y0} <= `tCQ {   Y3,Y2,Y1}; caseY <= 21; end
     `YFSM_MPT : if ( w2YQ && !jit   && !fetch) begin  `nYFSM(DOS);  {      Y1,Y0} <= `tCQ {      I1,I0}; caseY <= 22; end
            else if ( w2YQ && !jit   && !ckeY ) begin  `nYFSM(DOS);  {      Y1,Y0} <= `tCQ {      I1,I0}; caseY <= 23; end
      endcase
   end

assign   YQmt  =  YFSM[MPT]; // & ~jit;

// Data from YQ is always taken for the Execute datapath first and then for the Instruction Register
// To Instruction Register
assign   YQo      = |YFSM[QTR:UNO] && !rYQ &&            fi3YQ ?  {16'bx,Y0,1'b0}   // \
                  : |YFSM[QTR:DOS] &&  rYQ && !Prefix && fi3YQ ?  {16'bx,Y1,1'b0}   //  >  Format 3
                  : |YFSM[QTR:TRS] &&  rYQ &&  Prefix && fi3YQ ?  {16'bx,Y2,1'b0}   // /
                  : |YFSM[QTR:DOS] && !rYQ &&            fi1YQ ?  {   Y0,Y1,1'b1}   // \
                  : |YFSM[QTR:TRS] &&  rYQ && !Prefix && fi1YQ ?  {   Y1,Y2,1'b1}   //  >  Format 1
                  :  YFSM[QTR]     &&  rYQ &&  Prefix && fi1YQ ?  {   Y2,Y3,1'b1}   // /
                  :                                               {        33'bx};
// To Execute Datapath
assign   imm      =  Prefix   ?  {    Y0[15:0],Y1[15:0]}                     // If a Prefix then take two 'shorts'
                              :  {{16{Y0[15]}},Y0[15:0]};                    // Otherwise sign extend a single 'short'

assign   fn       =  Y0[WIMM-1:0];
assign   offset   =  imm;

// IQ  Instruction Queue ========================================================================================================

assign   format1  = ~P[1] | last;

assign   incoming[WINS+1:0]   =  format1 ? {CMo[31:0],2'b11} : {CMo[15:0],18'h000F1},  // lsb set TRUE to indicate First slo
         ifromYQ [WINS+1:0]   =  fi1YQ   ? {YQo[32:0],1'b1 } : {YQo[16:1],18'h000F1};

assign   nI[WINS+1:0]         =   IQmt &&  YQmt
                              ||  last &&  YQmt &&  jit
                              ||  last && !YQmt &&  jit ?  incoming[WINS+1:0]          // load new word from memory
                              :   last && !YQmt && !jit ?   ifromYQ[WINS+1:0]          // load new word from IQ
                              :  !last                  ?   sequent[WINS+1:0]          // Shift through current word
                              :                           { NOP,NOP,NOP,NOP,10'b0 };   // Load a null
// Instruction Register (I)
always @(posedge clock)
        if (ckeI &&  ( reset  ||  jump )) I[WINS+1:0] <= `tCQ    {WINS+2{1'b0}};         // Will cause IQmt
   else if (ckeI &&  ( wIQ    ||  rIQ  )) I[WINS+1:0] <= `tCQ  nI[WINS+1:0];

assign   order[WOPC-1:0]      =   I[WINS+1:WINS-WOPC+2];

// Examine the Pipe to see what can be excluded
// The A2S ISA specification allows 2-bit MTs to be treated as fillers and may therefore be skipped over.  If a single
// cycle delay is truly required then a 6-bit NOP must be explicitly specified at the assembler level.  This removal is done
// after the Pipe register is loaded to minimise the set-up into the register from memory.
// As we know where 2-bit NOPs are in the First cycle we only need detect 2 bits not the full expanded opcode

// Note: we could also detect what other ops could be combined -- swap, rots, nip, drop, dup etc...  next rev.??

//                                                               _____________format 1 / ~3
//                                                              /       ______first cycle
//                                                             /       /
assign   format1a = (I[03]  |  I[02]) &                     I[1] &  I[0],     // Format1 with  No  2-bit MT
         format1b = ~I[03]  & ~I[02]  &                     I[1] &  I[0],     // Format1 ends in a 2-bit MT which will be removed
         format1c =                                         I[1] & ~I[0],     // Format1 second cycle;  creates valid (Q) bits
         format3a = (I[27]  |  I[26]) & (I[19] |  I[18]) & ~I[1] &  I[0],     // slot1 isn't a 2-bit MT slot3 isn't a 2-bit MT
         format3b = (I[27]  |  I[26]) & ~I[19] & ~I[18]  & ~I[1] &  I[0],     // slot1 isn't a 2-bit MT slot3 is    a 2-bit MT
         format3c = ~I[27]  & ~I[26]  & (I[19] |  I[18]) & ~I[1] &  I[0],     // slot1 is    a 2-bit MT slot3 isn't a 2-bit MT
         format3d = ~I[27]  & ~I[26]  & ~I[19] & ~I[18]  & ~I[1] &  I[0];     // slot1 is    a 2-bit MT slot3 is    a 2-bit MT

// Expand 2-bit opcodes to full 6-bit opcodes from assembler:                       ^
// ops2bit  = {                        #  ab    expands to                          |
//       "MPT" : 0x0,                  #  00 -> xx xxxx  xx, x, x, x, x   -- treated separately
//       ":"   : 0x1, "call": 0x1,     #  01 -> 11 1001  11,~a,~b, a, b
//       "lit" : 0x2,                  #  10 -> 11 0110  11,~a,~b, a, b
//       "->r" : 0x3, "rtn" : 0x3 }    #  11 -> 11 0011  11,~a,~b, a, b

assign   xpslot5  =  {2'b11,~I[03],~I[02],I[03],I[02]};
assign   xpslot1  =  {2'b11,~I[27],~I[26],I[27],I[26]};
assign   xpslot3  =  {2'b11,~I[19],~I[18],I[19],I[18]};

// Create the Sequent: next value of Pipe as opcodes are shifted out.
// In First slot time after loading remove all 2-bit NOPs.  This works, as the first opcode is always used!

// Expand 2-bit opcodes within format 1 bundles.
//       I Register  33     27     21     15      9     3 2 1 0  State on first load of a format 1 bundle
//                   +------+======+------+------+------+--+-+-+
//    First cycle    |  op0 |  op1 |  op2 |  op3 |  op4 |o5|1|1| bit1: format-1 flag   bit0: first cycle flag
//                   +------+======+------+------+------+--+-+-+
//                   |  op1 |  op2 |  op3 |  op4 |  op5 | v010 | bit3: op5 is valid (v) i.e. not MT (empty)
//                   +------+======+------+------+------+------+
//                   |  op2 |  op3 |  op4 |  op5 | 00 111v 0000| bits7:3: bitmap of valid ops
//                   +------+======+------+------+------+------+
//                   |  op3 |  op4 |  op5 |  snb | 00 11v0 0000| bits7:3: bitmap of valid ops
//                   +------+======+------+------+------+------+
//                   |  op4 |  op5 |  snb |  snb | 00 1v00 0000| bits7:3: bitmap of valid ops
//                   +------+------+------+------+------+------+
//                   |  op5 |  snb |  snb |  snb | 00 v000 0000| bits7:3: bitmap of valid ops
//                   +------+------+------+------+------+------+
//                   |  snb |  snb |  snb |  snb | 00 0000 0000| Empty
//                   +------+------+------+------+------+------+
//
// Expand 2-bit opcodes within format 3 bundles.
//       I Register  33     27 25     19 17       9              State on first load of a format 3 bundle
//                   +------+--+------+--+-------+---------+-+-+
//       I Register  |  op0 |xp|  op2 |xp| xxxxx |000111100|0|1| bits7:3 bitmap of valid ops
//                   +------+--+------+--+-----------------+-+-+
//                   33     27     21     15      9       2 1 0  State on first load of a format 1 bundle
//    Second cycle   +------+======+------+------+-------------+
//       ~MT ~MT     |  op1 |  op2 |  op3 |  snb | 00 1110 0000| bits7:3 bitmap of valid ops
//                   +------+======+------+------+-------------+
//       ~MT  MT     |  op1 |  op2 |  snb |  snb | 00 1100 0000| bits7:3 bitmap of valid ops
//                   +------+======+------+------+-------------+
//        MT ~MT     |  op2 |  op3 |  snb |  snb | 00 1100 0000| bits7:3 bitmap of valid ops
//                   +------+======+------+------+-------------+
//        MT  MT     |  op2 |  snb |  snb |  snb | 00 1000 0000| bits7:3 bitmap of valid ops
//                   +------+------+------+------+-------------+
//
//     Third cycle   +------+======+------+------+-------------+
//       ~MT ~MT     |  op2 |  op3 |  snb |  snb | 00 1100 0000| bits7:3 bitmap of valid ops
//                   +------+======+------+------+-------------+
//       ~MT  MT     |  op2 |  snb |  snb |  snb | 00 1000 0000| bits7:3 bitmap of valid ops
//                   +------+------+------+------+-------------+
//        MT ~MT     |  op3 |  snb |  snb |  snb | 00 1000 0000| bits7:3 bitmap of valid ops
//                   +------+------+------+------+-------------+
//
//    Fourth cycle   +------+------+------+------+-------------+
//       ~MT ~MT     |  op3 |  snb |  snb |  snb | 00 1000 0000| bits7:3 bitmap of valid ops
//                   +------+------+------+------+-------------+
//
//       ======      Shows last instruction in slot0 as next op is an SNB (Skip to Next Bundle)
//                   SNB are used by the assembler as well as backfilling the IR as is done here


assign   sequent  =  format1a  ? {I[27:04],  xpslot5,                       4'b1010}   // op1 op2 op3 op4 ~nop
                  :  format1b  ? {I[27:04],  SNB,                           4'b0010}   // op1 op2 op3 op4  nop
                  :  format1c  ? {I[27:04],                   5'b00111,I[3],4'b0000}   // extend format1 add valid bits

                  :  format3a  ? {xpslot1, I[25:20], xpslot3, 16'b000000001110_0000}   // op1 op2 op3 nop  nop
                  :  format3b  ? {xpslot1, I[25:20], SNB,     16'b000000001100_0000}   // op1 op2 nop nop  nop
                  :  format3c  ? {I[25:20], xpslot3, SNB,     16'b000000001100_0000}   // op2 op3 nop nop  nop
                  :  format3d  ? {I[25:20], SNB, SNB,         16'b000000001000_0000}   // op2 nop nop nop  nop
                  :              {I[27:10], SNB,              2'b00,I[6:4],5'b00000};  // Simple Shift-left ops: all other cycles

assign   IQmt     =   I[7:0]   == 8'b0000_0000,                                        // Instruction Queue is empty
         last1    =   I[7:0]   == 8'b1000_0000,                                        // Normal finish
         last2    =   I[27:22] == SNB,                                                 // Early finish format1s and format3a,3b
         last3    =  {I[27:26],I[25:20],I[1:0]} == {2'b00,SNB,2'b01},                  // Early finish format3c,3d
         last     =   last1 | last2 | last3;

//always @(posedge clock) if ( ckePfx ) Prefix <= `tCQ ~reset & ((order==PFX) | Prefix & ~rYQ); //JLPL - 10_24_25 -> Comment out as this was an ifndef A2_ENABLE_INTERRUPTS

// Instruction Flow =============================================================================================================
// Straight Decodes -------------------------------------------------------------------------------------------------------------

assign
   rDM_order      =  (order==GETA) | (order==GETB) | (order==GETC) | (order==GET )
                  |  (order==GTAP) | (order==GTBP) | (order==GTCP) | (order==XCHG),
   wDM_order      =  (order==PUTA) | (order==PUTB) | (order==PUTC) | (order==PUT )     // Data Memory Write
                  |  (order==PTAP) | (order==PTBP) | (order==PTCP) | (order==XCHG),
   xDM_order      =  (order==XCHG),
   eDM_order      =   next         & (rDM_order    |  wDM_order),
   lit_order      =  (order==ZERO) | (order==ONE ) | (order==LIT ),
   T0addr_order   =  (order==PUT)  | (order==GET)  | (order==XCHG),                    // Data Memory address from T0

   jmp2RSo_order  =  (order==RTN)  | (order==CO),
   prefix_order   =  (order==PFX),

// Overlap order are ones that may continue in parallel with a memory read access
// If we were clever we could include over,swap,rot3,rot4,nip,drop which ONLY move the destination of incoming reads!!!
// If we were REALLY clever could also include T2A, T2B, T2C and T2R
// If we were DEVASTATINGLY clever could also include some FN
   overlap_order  =   next & ~IQmt & ~(DMreading & DMwriting)                          // No overlaps on XCHG
                  & ((order==GETA) | (order==GETB) | (order==GETC) | (order==GET )     // Any other memory read
                  |  (order==GTAP) | (order==GTBP) | (order==GTCP) | (order==res1)     // :
                  |  (order==A2T ) | (order==B2T ) | (order==C2T ) | (order==R2T )     // Any ABC to T
                  |  (order==SNB ) | (order==CO  ) | (order==NOP ) | (order==RTN )     // Ancillary
                  |  (order==ZERO) | (order==ONE ) | (order==LIT ) | (order==CALL)     // Literals or CALL
                  |  (order==PFX )                                                     // Prefix
                  |  (order==JANZ) | (order==JBNZ) | (order==JCNZ) | (order==JMP )),   //

   eFN_order      =  (order==EXT)  & ~YQmt;                                            // Output to Extension Functions

// FSM --------------------------------------------------------------------------------------------------------------------------
assign   fail     =  (order==JZ)   &  (T0!=IMM0)
                  |  (order==JN)   &  ~T0[WA2S-1]
                  |  (order==JANZ) &   Az
                  |  (order==JBNZ) &   Bz
                  |  (order==JCNZ) &   Cz;

assign   kodd     =  CMk   &  P[1],                                                    // Incoming bundle on odd  boundary
         kevn     =  CMk   & ~P[1],                                                    // Incoming bundle on even boundary
         lodd     =  last  &  P[1],                                                    // Last in  bundle on odd  boundary
         levn     =  last  & ~P[1],                                                    // Last in  bundle on even boundary
         lont     =  last  &  P[1] & fail,                                             // Last-odd-not-taken
         lent     =  last  & ~P[1] & fail;                                             // Last-even-not-taken

`define  FSM_NEXT default
`define  COND     JZ,JANZ,JBNZ,JCNZ,JN
//`define  MEMA     PUTA,PUTB,PUTC,PUT,GETA,GETB,GETC,GET,PTAP,PTBP,PTCP,XCHG,GTAP,GTBP,GTCP,PTMA,PTMB,PTMC
// Main Finite State Machine
always @*  (* parallel_case *) case (1'b1)
`define                           CP   {rIQ,jump,snP, ldP,cR,rYQ,ex,ldF,nFSM}
//                                       1  1    2    1   2    1  1  1  3
   FSM[START] : if (reset) begin `CP = { l, l,  sP4,  l, HOLD, l, l, h, START }; caseI = 8'd00; end  //
           else            begin `CP = { l, h,   xx,  h, HOLD, l, l, h, VECTOR}; caseI = 8'd01; end  //
   FSM[VECTOR]:            begin `CP = { l, h,   xx,  h, HOLD, h, l, h, FETCH }; caseI = 8'd03; end  //
// FSM[VECTOR]: if (kevn)  begin `CP = { l, h,   xx,  h, HOLD, h, l, h, FETCH }; caseI = 8'd03; end  // Forced to even boundary
//         else            begin `CP = { l, l,   xx,  l, HOLD, l, l, l, VECTOR}; caseI = 8'd04; end  //
   FSM[FETCH] : if (Intpt) begin `CP = { l, h,  sP0,  h, PUSH, l, h, h, VECTOR}; caseI = 8'd05; end  // PUSH P & vector to entry-point
           else if (kodd)  begin `CP = { l, l,  sP2,  h, HOLD, l, l, h, NEXT  }; caseI = 8'd06; end  //
           else if (kevn)  begin `CP = { l, l,  sP4,  h, HOLD, l, l, h, NEXT  }; caseI = 8'd07; end  //
           else            begin `CP = { l, l,   xx,  l, HOLD, l, l, l, FETCH }; caseI = 8'd08; end  //
  `FSM_NEXT   : if (!IQmt) (* parallel_case *) case (order)                                          // FSM[NEXT] ***************
      // Literals -----------------------------------------------------------------------------
      PFX   : if (YQmt)    begin `CP = { l, l,   xx,  l, HOLD, l, l, l, NEXT  }; caseI = 8'd09; end  //
         else              begin `CP = { h, l,  sP2,  h, HOLD, l, l, l, NEXT  }; caseI = 8'd10; end  //
      LIT   : if (YQmt)    begin `CP = { l, l,   xx,  l, HOLD, h, l, l, NEXT  }; caseI = 8'd11; end  //
         else if (lodd)    begin `CP = { h, l,  sP6,  h, HOLD, h, h, l, NEXT  }; caseI = 8'd12; end  //
         else if (levn)    begin `CP = { h, l,  sP4,  h, HOLD, h, h, l, NEXT  }; caseI = 8'd13; end  //
         else              begin `CP = { h, l,  sP2,  h, HOLD, h, h, l, NEXT  }; caseI = 8'd14; end  //
      // Labels & Extends ---------------------------------------------------------------------
      EXT   : if (YQmt)    begin `CP = { l, l,   xx,  l, HOLD, h, l, l, NEXT  }; caseI = 8'd15; end  // Wait for function immediate
         else if (lodd)    begin `CP = { h, l,  sP6,  h, HOLD, h, h, l, NEXT  }; caseI = 8'd16; end  //
         else if (levn)    begin `CP = { h, l,  sP4,  h, HOLD, h, h, l, NEXT  }; caseI = 8'd17; end  // ** test **
         else              begin `CP = { h, l,  sP2,  h, HOLD, h, h, l, NEXT  }; caseI = 8'd18; end  // ** test **
      CALL  : if (YQmt)    begin `CP = { l, l,  sP0,  l, HOLD, h, l, l, NEXT  }; caseI = 8'd19; end  // Wait for label
         else              begin `CP = { h, h,  sP2,  h, PUSH, h, h, h, FETCH }; caseI = 8'd20; end  // Non-seq  fetch
      JMP   : if (YQmt)    begin `CP = { l, l,   xx,  l, HOLD, h, l, l, NEXT  }; caseI = 8'd21; end  // Wait for label
         else              begin `CP = { l, h,   xx,  h, HOLD, h, h, h, FETCH }; caseI = 8'd22; end  // Non-seq  fetch
      // Conditionals -------------------------------------------------------------------------
     `COND  : if (YQmt)    begin `CP = { l, l,   xx,  l, HOLD, h, l, l, NEXT  }; caseI = 8'd23; end  // Wait for offset **1
         else if (lont)    begin `CP = { h, l,  sP6,  h, HOLD, h, h, l, NEXT  }; caseI = 8'd24; end  // Last odd  Branch NOT taken *2
         else if (lent)    begin `CP = { h, l,  sP4,  h, HOLD, h, h, l, NEXT  }; caseI = 8'd25; end  // Last even Branch NOT taken *3
         else if (fail)    begin `CP = { h, l,  sP2,  h, HOLD, h, h, l, NEXT  }; caseI = 8'd26; end  // Not Last  Branch NOT taken
         else              begin `CP = { l, h,   xx,  l, HOLD, h, h, h, FETCH }; caseI = 8'd27; end  // Branch Taken -- non-seq fetch
      // No Immediates ------------------------------------------------------------------------
      RTN   :              begin `CP = { l, h,  sP0,  h, POP,  l, h, h, FETCH }; caseI = 8'd28; end  // Non-seq fetch
      CO    :              begin `CP = { l, h,  sP0,  l, RPLC, l, h, h, FETCH }; caseI = 8'd29; end  // Non-seq fetch
      T2R   : if (lodd)    begin `CP = { h, l,  sP2,  h, PUSH, l, h, l, NEXT  }; caseI = 8'd30; end  // Last and odd
         else if (levn)    begin `CP = { h, l,  sP4,  h, PUSH, l, h, l, NEXT  }; caseI = 8'd31; end  // Last and even
         else              begin `CP = { h, l,   xx,  l, PUSH, l, h, l, NEXT  }; caseI = 8'd32; end  // NOT last
      R2T   : if (lodd)    begin `CP = { h, l,  sP2,  h, POP,  l, h, l, NEXT  }; caseI = 8'd33; end  // Last and odd
         else if (levn)    begin `CP = { h, l,  sP4,  h, POP,  l, h, l, NEXT  }; caseI = 8'd34; end  // Last and even
         else              begin `CP = { h, l,   xx,  l, POP,  l, h, l, NEXT  }; caseI = 8'd35; end  // NOT last
      default if (lodd)    begin `CP = { h, l,  sP2,  h, HOLD, l, h, l, NEXT  }; caseI = 8'd47; end  // Last and odd
         else if (levn)    begin `CP = { h, l,  sP4,  h, HOLD, l, h, l, NEXT  }; caseI = 8'd48; end  // Last and even
         else              begin `CP = { h, l,   xx,  l, HOLD, l, h, l, NEXT  }; caseI = 8'd49; end  // NOT last
      endcase //-------------------------------------------------------------------------------      // case(order)
      else                 begin `CP = { l, l,   xx,  l, HOLD, l, l, l, NEXT  }; caseI = 8'd255;end  // IQ empty
   endcase //----------------------------------------------------------------------------------      // case(1'b1)

   // **1   Not-taken jumps do not use the offset from YQ; but even so, if the offset ia not ready (YQmt) we must wait for it
   //       so we can remove it.

always @(posedge clock)
   if       (reset)  FSM  <= `tCQ  4'b1 << START;
   else if  ( ldF )  FSM  <= `tCQ  4'b1 << nFSM;

assign   start    =  FSM[START],
         fetch    =  FSM[FETCH],
         vector   =  FSM[VECTOR],
         next     =  FSM[NEXT],
         activ    = !start;

// wire     clash    =  next & Intpt & (order==RTN);

always @*
         wIQ      =  fetch;

// Address Generation and A,B & C registers =====================================================================================

assign   asel[     6:0] = DMreading && DMwriting ? 7'h60 : (32 * order[1:0]);
assign   addr[WA2S-1:0] = {T0,C,B,A} >>  asel; // (32 * order[1:0]);       // Memory Address Multiplexer
assign   sABC[     2:0] = 3'b001     <<  order[1:0];

// Categorize  Opcodes into their respective actions on A,B or C, if any
always @* (* parallel_case *) casez (order)
   GETA,GETB,GETC,PUTA,PUTB,PUTC : begin  nABC =  addr + IMM0;  {ldC,ldB,ldA} = 3'b00;                end // addr + 00000000
   GTAP,GTBP,GTCP,PTAP,PTBP,PTCP : begin  nABC =  addr + NBPW;  {ldC,ldB,ldA} = sABC;                 end // addr + 00000004
   DECA,DECB,DECC                : begin  nABC =  addr + IBPW;  {ldC,ldB,ldA} = sABC;                 end // addr + fffffffc
   JANZ,JBNZ,JCNZ                : begin  nABC =  addr + NEG1;  {ldC,ldB,ldA} = sABC & {~Cz,~Bz,~Az}; end // addr + ffffffff [*1]
   T2A, T2B, T2C                 : begin  nABC =  T0;           {ldC,ldB,ldA} = sABC;                 end
   default                         begin  nABC =  IMM0;         {ldC,ldB,ldA} = 3'b000;               end
   endcase
// Note [*1]:  For JDxNZs when the count goes to zero the final pass through does NOT decrement to minus 1
//             For a count of 4 load A,B,C with 3 and we end on zero and A,B,C will stay at 0

assign   ABCz  =  nABC == IMM0;

always @(posedge clock) if (ckeABC && ldA)  { A,Az } <= `tCQ { nABC,ABCz };
always @(posedge clock) if (ckeABC && ldB)  { B,Bz } <= `tCQ { nABC,ABCz };
always @(posedge clock) if (ckeABC && ldC)  { C,Cz } <= `tCQ { nABC,ABCz };

// Execute Datapath =============================================================================================================
// Execute function creating nT0 for next T0,  nT1 and nT2 are the new data for T1 and T2.  T3 only gets data from T2 on a net
// push.  wT are the write enables to T0..3 and Tm is the mode of Pops and Pushes for the operation.
// Refilling the Tstack is controlled by its own state machine.

`define READS  GETA,GETB,GETC,GTAP,GTBP,GTCP, DUP
`define POPS   PUTA,PUTB,PUTC,PTAP,PTBP,PTCP, DROP,T2A,T2B,T2C,T2R

localparam [3:0]  NOST = 4'd0, P0P1 = 4'd1, P1P0 = 4'd2, P1P1 = 4'd3, P2P0 = 4'd4, // Notation PxPy is Pop(x),Push(y)
                  P2P1 = 4'd5, P2P2 = 4'd6, P3P1 = 4'd7, P3P3 = 4'd8, P4P4 = 4'd9;

assign sub  =  order[0];

always @(posedge clock) if (next) X2 <= `tCQ (order==XCHG);

`define                     xTD   { nT0[WA2S-1:0],            pop2,sPush,wT[3:0], Tm[3:0]}
//                                   |                          |___   |   |       |
always @* if (X2)    begin `xTD = {  T0,                            l, l, 4'b0111, P2P1  }; caseD = 32; end // Atomic Exchange PART 2
   else if (next && !IQmt) (* parallel_case *) case (order) //      |  |   |       |
  `READS           : begin `xTD = {  T0,                            l, h, 4'b1111, P0P1  }; caseD = 00; end // Push
   XCHG            : begin `xTD = {  T0,                            l, h, 4'b0000, NOST  }; caseD = 01; end // Atomic Exchange  wxyz wxa
   GET             : begin `xTD = {  T0,                            l, l, 4'b0000, P1P1  }; caseD = 02; end // Swap addr for read data
   A2T,B2T,C2T     : begin `xTD = {  addr,                          l, h, 4'b1111, P0P1  }; caseD = 03; end // Address
   R2T             : begin `xTD = {  RSo,                           l, h, 4'b1111, P0P1  }; caseD = 04; end // R stack
   AND             : begin `xTD = {  T1 & T0,                       l, l, 4'b0111, P2P1  }; caseD = 05; end // Bitwise
   XOR             : begin `xTD = {  T1 ^ T0,                       l, l, 4'b0111, P2P1  }; caseD = 06; end // :
   IOR             : begin `xTD = {  T1 | T0,                       l, l, 4'b0111, P2P1  }; caseD = 07; end // :
   NOT             : begin `xTD = { ~T0,                            l, l, 4'b0001, P1P1  }; caseD = 08; end // :
   OVER            : begin `xTD = {  T1,                            l, h, 4'b1111, P0P1  }; caseD = 09; end // Over
   SWAP            : begin `xTD = {  T1,                            l, h, 4'b0011, P2P2  }; caseD = 10; end // Swap
   ROT3            : begin `xTD = {  T2,                            l, h, 4'b0111, P3P3  }; caseD = 11; end // Rotate 3
   ROT4            : begin `xTD = {  T3,                            l, h, 4'b1111, P4P4  }; caseD = 12; end // Rotate 4
   SEL             : begin `xTD = {  T0 == IMM0 ? T1 : T2,          h, l, 4'b0011, P2P0  }; caseD = 13; end // POP2
   ADD,SUB         : begin `xTD = {  T1 + ({WA2S{sub}} ^ T0) + sub, l, l, 4'b0111, P2P1  }; caseD = 14; end // Add / Sub.
   NIP             : begin `xTD = {  T0,                            l, l, 4'b0110, P1P0  }; caseD = 15; end // NIP
  `POPS            : begin `xTD = {  T1,                            l, l, 4'b0111, P1P0  }; caseD = 16; end // POP
   PUT             : begin `xTD = {  T2,                            h, l, 4'b0011, P2P0  }; caseD = 17; end // POP2
   EQ              : begin `xTD = {  T1 == T0      ? TRUE:FALSE,    l, l, 4'b0111, P2P1  }; caseD = 18; end // Compare
   SLT             : begin `xTD = {`S(T1) < `S(T0) ? TRUE:FALSE,    l, l, 4'b0111, P2P1  }; caseD = 19; end // :
   ULT             : begin `xTD = {   T1  <    T0  ? TRUE:FALSE,    l, l, 4'b0111, P2P1  }; caseD = 20; end // :
   ZERO,ONE        : begin `xTD = { {{WA2S-1{1'b0}},order[0]},      l, h, 4'b1111, P0P1  }; caseD = 21; end // Lit 0, 1
   JZ,JN : if (YQmt) begin `xTD = {  T1,                            l, l, 4'b0000, NOST  }; caseD = 22; end // empty!
           else      begin `xTD = {  T1,                            l, l, 4'b0111, P1P0  }; caseD = 23; end // POP
   LIT   : if (YQmt) begin `xTD = {  imm,                           l, h, 4'b0000, NOST  }; caseD = 24; end // empty!
           else      begin `xTD = {  imm,                           l, h, 4'b1111, P0P1  }; caseD = 25; end // Immediate
   EXT   : if (YQmt) begin `xTD = {  IMM0,                          l, h, 4'b0000, NOST  }; caseD = 26; end // empty!
           else      begin `xTD = {  IMM0,                          l, h, wX[3:0],Xm[3:0]}; caseD = 27; end // leave it!!
   default         : begin `xTD = {  IMM0,                          l, h, 4'b0000, NOST  }; caseD = 28; end // the rest..
   endcase
   else              begin `xTD = {  IMM0,                          l, h, 4'b0000, NOST  }; caseD = 29; end // empty!

// Function results for single-cycle functions provide the result as a part of xT0 and fit the main scheme.
// Function results that take multiple cycles allow other instructions to proceed as long as they do not require the
// operand being generated.  These results are inserted into T1,T2 or T3 and enabled by wF[3:0].
//
// Further memory reads may be delayed.  As long as T0 ( the memory read target) is not used by a subsequent instruction then
// overlapped execution may continue.  The tracking of the stack and the eventual DMk (memory read acknowledge) create the
// signals k[0..3] to force the load of the Tstack with data from memory into the appropriate location.
//
// Top 4 of Data Stack ==========================================================================================================

assign   ldT[3:0]  =  (wX[3:0] | wT[3:0]) & nTdepth[3:0];
//
// 'run' is the inverse of stall.  'ex' is valid-for-execute (e.g. !YQmt on a LIT)
//
always @(posedge clock) if (k[0])   T0 <= `tCQ DMo;                 // Load read data from memory
         else if (          s[0])   T0 <= `tCQ DSo;                 // Reload from data stack
         else if (ckeT && ldT[0])   T0 <= `tCQ wX[0] ? nF0 : nT0;   // Normal operation from Extensions or Main

always @(posedge clock) if (k[1])   T1 <= `tCQ DMo;
         else if (          s[1])   T1 <= `tCQ DSo;
         else if (ckeT && ldT[1])   T1 <= `tCQ wX[1] ? nF1 : (sPush ? T0 : (pop2 ? T3 : T2));

always @(posedge clock) if (k[2])   T2 <= `tCQ DMo;
         else if (          s[2])   T2 <= `tCQ DSo;
         else if (ckeT && ldT[2])   T2 <= `tCQ wX[2] ? nF2 : (sPush ? T1 : T3);

always @(posedge clock) if (k[3])   T3 <= `tCQ DMo;
         else if (ckeT && ldT[3])   T3 <= `tCQ T2;

// T Stack Control ==============================================================================================================
// The variables M,N & Q control the greediness of the T stack pulling from the main stack when data is popped from the Tstack
// If M  is asserted the stack will try to prefetch so that at least 1 item  is  on the Tstack
// If N  is asserted the stack will try to prefetch so that at least 2 items are on the Tstack
// If Q  is asserted the stack will try to prefetch so that at least 3 items are on the Tstack
// Of course, if the main stack is empty this action fails and the Tdepth adjusted accordingly

// NOTA BENE:  If the main stack is empty and a Pop is generated here then.......several different actions......
localparam [3:0]  Td0 = 4'b0000,  Td1  = 4'b0001, Td2  = 4'b0011, Td3  = 4'b0111, Td4  = 4'b1111;

wire [3:0]  Sm =  eFN_order ? Xm : Tm;

assign {M,N,Q} =  3'b100;                          // Should move over to Status Register when Interrupts included

// Tstack backfill from Main Stack (DS)                      ________________ Write to Tdepth with nTdepth
//                                                          /  ______________ stall to reload T from S
//                                                         /  /   ___________ location in T0,T1,T2 to restore from D stack
//                                                        /  /   /      _____ Command to D-stack LIFO
always @* (* parallel_case *) case (Tdepth)//            /  /   /      /
`define                                    XD {nTdepth,wTd,Ts,s[2:0], cD   }
   Td0   :  case (Sm)                      //      |    |  |  |       |
      P0P1        :  if (N && !Dmt) begin `XD =  { Td2, h, l, 3'b010, POP  }; caseT = 00; end
                     else           begin `XD =  { Td1, h, l, 3'b000, HOLD }; caseT = 01; end
      P1P0        :                 begin `XD =  { Td1, h, h, 3'b001, POP  }; caseT = 02; end
      P2P1        :                 begin `XD =  { Td1, h, h, 3'b001, POP  }; caseT = 03; end
      default                       begin `XD =  { Td0, l, l, 3'b000, HOLD }; caseT = 04; end endcase  // stall execution
   Td1   :  case (Sm)
      P0P1        :  if (Q && !Dmt) begin `XD =  { Td3, h, l, 3'b100, POP  }; caseT = 05; end
                     else           begin `XD =  { Td2, h, l, 3'b000, HOLD }; caseT = 06; end
      P1P1        :  if (N && !Dmt) begin `XD =  { Td2, h, l, 3'b010, POP  }; caseT = 07; end
                     else           begin `XD =  { Td1, h, l, 3'b000, HOLD }; caseT = 08; end
      P1P0        :  if (M && !Dmt) begin `XD =  { Td1, l, l, 3'b001, POP  }; caseT = 09; end
                     else           begin `XD =  { Td0, h, l, 3'b000, HOLD }; caseT = 10; end
      P2P1        :                 begin `XD =  { Td2, h, h, 3'b010, POP  }; caseT = 11; end
      NOST        :                 begin `XD =  { Td1, l, l, 3'b000, HOLD }; caseT = 12; end
      default                       begin `XD =  { Td2, h, h, 3'b010, POP  }; caseT = 13; end endcase  // stall execution
   Td2   :  case (Sm)
      P3P1, P4P4  :                 begin `XD =  { Td3, h, h, 3'b100, POP  }; caseT = 14; end          // stall execution
      P3P3        :                 begin `XD =  { Td3, h, l, 3'b001, POP  }; caseT = 15; end          // rot3 completes!
      P0P1        :                 begin `XD =  { Td3, h, l, 3'b000, HOLD }; caseT = 16; end
      P2P0        :  if (M && !Dmt) begin `XD =  { Td1, h, l, 3'b001, POP  }; caseT = 17; end          // Pop2
                     else           begin `XD =  { Td0, h, l, 3'b000, HOLD }; caseT = 18; end
      P1P0, P2P1  :  if (N && !Dmt) begin `XD =  { Td2, l, l, 3'b010, POP  }; caseT = 19; end
                     else           begin `XD =  { Td1, h, l, 3'b000, HOLD }; caseT = 20; end
      P1P1, P2P2  :  if (Q && !Dmt) begin `XD =  { Td3, h, l, 3'b100, POP  }; caseT = 21; end
                     else           begin `XD =  { Td2, l, l, 3'b000, HOLD }; caseT = 22; end
      default                       begin `XD =  { Td2, l, l, 3'b000, HOLD }; caseT = 23; end endcase
   Td3   :  case (Sm)
      P2P0, P3P1  :  if (N && !Dmt) begin `XD =  { Td2, h, l, 3'b010, POP  }; caseT = 24; end
                     else           begin `XD =  { Td1, h, l, 3'b000, HOLD }; caseT = 25; end
      P1P0        :  if (Q && !Dmt) begin `XD =  { Td3, h, l, 3'b100, POP  }; caseT = 26; end
                     else           begin `XD =  { Td2, h, l, 3'b000, HOLD }; caseT = 27; end
      P2P1        :  if (Q && !Dmt) begin `XD =  { Td3, h, l, 3'b100, POP  }; caseT = 28; end
                     else           begin `XD =  { Td2, h, l, 3'b000, HOLD }; caseT = 29; end
      P0P1        :                 begin `XD =  { Td4, h, l, 3'b000, HOLD }; caseT = 30; end
      P4P4        :                 begin `XD =  { Td4, h, l, 3'b001, HOLD }; caseT = 31; end        // rot4 completes
      P1P1, P2P2, P3P3  :           begin `XD =  { Td3, l, l, 3'b000, HOLD }; caseT = 32; end
      default                       begin `XD =  { Td3, l, l, 3'b000, HOLD }; caseT = 33; end endcase
   Td4   :  case (Sm)
      P3P1, P2P0  :  if (Q && !Dmt) begin `XD =  { Td3, h, l, 3'b100, POP  }; caseT = 34; end
                     else           begin `XD =  { Td2, h, l, 3'b000, HOLD }; caseT = 35; end
      P1P0, P2P1  :                 begin `XD =  { Td3, h, l, 3'b000, HOLD }; caseT = 36; end
      P0P1        :                 begin `XD =  { Td4, l, l, 3'b000, PUSH }; caseT = 37; end
      default                       begin `XD =  { Td4, l, l, 3'b000, HOLD }; caseT = 38; end endcase
   default                          begin `XD =  { Td0, l, l, 3'b000, HOLD }; caseT = 39; end      // Invalid condition
   endcase

always @(posedge clock) if (reset) Tdepth  <= `tCQ Td0;
   else if ((X2 || ckeTd) &&  wTd) Tdepth  <= `tCQ nTdepth;

// Main Stacks ==================================================================================================================

assign   iD[WA2S-1:0]   =  T3,
         iR[WA2S-1:0]   = ~reset && next && (order==T2R) ?  T0 : advP;

// Data Stack
A2Sv2r3_LIFO #(WA2S,DDSK) iSD (
   // outputs
   .LFo  (DSo),
   .LFd  (Ddepth),
   .LFf  (Dfull),
   .LFm  (Dmt),
   .LFs  (Dstall),
   // inputs
   .iLF  (iD),
   .cLF  (cD),
   .eLF  (ckeDstk),
   .sLF  (spilD),
   .fLF  (fillD),
   .clock(clock),
   .reset(reset));

// Return Stack
A2Sv2r3_LIFO #(WA2S,DRSK) iSR (
   // outputs
   .LFo  (RSo),
   .LFd  (Rdepth),
   .LFf  (Rfull),
   .LFm  (Rmt),
   .LFs  (Rstall),
   // inputs
   .iLF  (iR),
   .cLF  (cR),
   .eLF  (ckeRstk),
   .sLF  (spilR),
   .fLF  (fillR),
   .clock(clock),
   .reset(reset));

// Outputs ======================================================================================================================
// Memory accesses, as far as A2S is concerned, will NOT stall for anything other than an inactive grant signal or an expected
// acknowledge which will only happen in the following cycle (& instruction).  So the outgoing eDM to request an access does not
// need to be qualified by A2S internal stalls. DMg is used externally to enable an access to begin.

// Code Memory
assign   eCM            = ~reset & (jump | seqreq),
         aCM[WA2S-1:0]  =  nO[WA2S-1:0];

// Data Memory
assign   eDM            =  next &  ~IQmt &  ~Ts &  eDM_order,
         rDM            =  next &  ~IQmt &  ~Ts &  rDM_order,                          // Memory Reads   or Exchange
         wDM            =  next &  ~IQmt &  ~Ts &  wDM_order,                          // Memory Writes  or Exchange
         aDM[WA2S-1:0]  =  next && !IQmt && !Ts && eDM_order
                        || DMreading     && DMwriting             ? addr : {WA2S{1'b0}},
         iDM[WA2S-1:0]  =  next && !IQmt && !Ts && T0addr_order
                        || DMreading     && DMwriting             ? T1
                        :  next && !IQmt && !Ts && wDM_order      ? T0   : {WA2S{1'b0}}; // Write T1 if T0 is an address!

// Input\Output
assign   iIO[WA2S-1:0]  =  T0[WA2S-1:0],
         eIO            = ~fn[15] &  eFN_order & ~stallIO,
         cIO[ 1:0]      =  fn[14:13],
         aIO[12:0]      =  fn[12:00];

// Collate Buses for output------------------------------------------------------------------------------------------------------
//                               Request,Write,WriteData,Address
//                                    66,   65,  64, 63:32, 31:0
assign   dmosi[`DMOSI_WIDTH-1:0] =  {eDM,  wDM, rDM,   iDM,  aDM };
assign   cmosi[        WA2S  :0] =  {eCM,                    aCM };
assign   imosi[`IMOSI_WIDTH-1:0] =  {eIO,  cIO,        iIO,  aIO };
//                                    47, 46:45,     44:13, 12:0

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//    END OF CORE   /////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//    EXTENSION FUNCTIONS    ////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`include "A2Sv2r3_Extensions_TZ.v"
`include "A2Sv2r3_Interrupts_TZ.v"

//`define  SXP(x)  {{WA2S-LA2S+1{x[LA2S]}},x[LA2S-1:0]}

// FUNCTION DATAPATH Collect all Extension Function results
always @* if (eFN_order) (* parallel_case *) casez (fn)

`define                              xFD   { nF0,            nF1,          nF2, wX[3:0], Xm[3:0]}
// I/O  <IO functions are always included>   |               |             |      |     |
   IORC,IORD               :  begin `xFD = { irdd,           T0,           T1, 4'b1111, P0P1  };  caseF = 00; end
   IOSW                    :  begin `xFD = { irdd,           T1,           X,  4'b0001, P1P1  };  caseF = 01; end
   IOWR                    :  begin `xFD = { T1,             T2,           T3, 4'b0111, P1P0  };  caseF = 02; end

   RORI,ROLI,LSRI,LSLI,ASRI:  begin `xFD = { shift,          X,            X,  4'b0001, P1P1  };  caseF = 03; end
   ROR, ROL, LSR, LSL, ASR :  begin `xFD = { shift,          T2,           T3, 4'b0111, P2P1  };  caseF = 04; end

   BSWP                    :  begin `xFD = { swapb[31:0],     X,           X,  4'b0001, P1P1  };  caseF = 08; end

   BREV                    :  begin `xFD = { rev[31:0],       X,           X,  4'b0001, P1P1  };  caseF = 09; end //JLPL - 10_23_25

   SHFL                    :  begin `xFD = { shuf[31:0],     shuf[63:32],  X,  4'b0011, P2P2  };  caseF = 10; end
   CC                      :  begin `xFD = { cctest,         T0,           T1, 4'b1111, P0P1  };  caseF = 11; end

   POPC,CLZ,CTZ            :  begin `xFD = { popsum,         T1,           X,  4'b0001, P1P1  };  caseF = 12; end
   EXCS                    :  begin `xFD = { popsum,         T2,           T3, 4'b0111, P2P1  };  caseF = 13; end

   MPY                     :  begin `xFD = { MDo[63:32],     MDo[31:00],   X,  4'b0011, P2P2  };  caseF = 20; end
   MPH                     :  begin `xFD = { MDo[63:32],     T2,           T3, 4'b0111, P2P1  };  caseF = 21; end
   MPL                     :  begin `xFD = { MDo[31:00],     T2,           T3, 4'b0111, P2P1  };  caseF = 22; end
   DIV                     :  begin `xFD = { MDo[31:00],     MDo[63:32],   X,  4'b0011, P2P2  };  caseF = 23; end
   REM                     :  begin `xFD = { MDo[63:02],     T2,           T3, 4'b0111, P2P1  };  caseF = 24; end
   QUO                     :  begin `xFD = { MDo[31:00],     T2,           T3, 4'b0111, P2P1  };  caseF = 25; end
   default                    begin `xFD = { X,              X,            X,  4'b0000, NOST  };  caseF =254; end
   endcase
   else                       begin `xFD = { X,              X,            X,  4'b0000, NOST  };  caseF =255; end

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////  END OF EXTENSION FUNCTIONS ////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// Collect all I/O read data
assign   irdd[WA2S-1:0] =  IOo[WA2S-1:0]        // I/O Input Read Data
                        |  IOo_intpt
                        ;  // Leave this semicolon here!!!

// ... and acknowledges
assign           iack   =  IOk                  // I/O Input acknowledge
                        |  IOk_intpt
                        ;  // Leave this semicolon here!!!

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//    END OF A2S CORE    ////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   VERIFY_A2S

`define  aSCR        13'h1555             // Scratch Register
`define  aCOM1       13'h03f8             // COM 1

module A2S_v2_wRAMs  #(
   parameter   DCMB  = 524288,            // A2S Depth of Code Memory in Bytes
   parameter   DDMB  = 524288,            // A2S Depth of Data Memory in Bytes
   parameter   WA2S  = 32,                // A2S Machine Width
   parameter   WINS  = 32,                // A2S Instruction Word Width
   parameter   WIMM  = 16,                // A2S Width of Immediates
   parameter   DRSK  = 32,                // A2S Depth of Return Stack
   parameter   DDSK  = 32,                // A2S Depth of Data Stack
   parameter   NCCI  = 10
   )(
   output wire [WIMM+WA2S-1 :0]  imosi,   // I/O Register Master-out slave-in
   input  wire [WA2S        :0]  imiso,   // I/O Register Master-in slave-out

   input  wire [2*WA2S+1    :0]  bmosi,   // Boot Load Code Memory

`ifdef  A2S_ENABLE_INTERRUPTS
   input  wire                   intpt,
`endif
`ifdef  A2S_ENABLE_CONDITION_CODES
   input  wire [NCCI-1      :0]  ccode,
`endif
   input  wire                   reset,
   input  wire                   fstck,
   input  wire                   clock
   );

localparam  DCMW  =  DCMB / 4;            // Depth of Code Memory in Words
localparam  DDMW  =  DDMB / 4;            // Depth of Data Memory in Words
localparam  MACM  = (`LG2(DCMB))-1;       // MS address bit
localparam  MADM  = (`LG2(DDMB))-1;       // MS address bit
localparam  [2:0] IDLE = 0, LAST = 7;

wire [3+ 2*WA2S-1:0] dmosi;               // Data Memory  Master-out slave-in
wire [     WA2S+1:0] dmiso;               // Data Memory  Master-in slave-out
wire [     WA2S  :0] cmosi;               // Code Memory  Master-out slave-in
wire [     WA2S+1:0] cmiso;               // Code Memory  Master-out slave-in
`ifdef   A2S_ENABLE_SIMD
wire [      290-1:0] wmosi;               // Work Memory  Master-out slave-in  290 {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]}
reg  [      130-1:0] wmiso;               // Work Memory  Master-in slave-out  130 {WMg, WMk, WMo[127:0]}
`endif //A2S_ENABLE_SIMD
wire                 alive;
wire                 intpt;
wire                 gclock;

A2Sv2r3 #(
   .WA2S(32),                             // A2S Machine Width
   .WINS(32),                             // A2S Instruction Word Width
   .DRSK(32),                             // A2S Depth of Return Stack
   .DDSK(32)                              // A2S Depth of Data Stack
   ) iA2S (
   .dmosi(dmosi),                         // Data Memory   Master-out slave-in
   .dmiso(dmiso),                         // Data Memory   Master-in slave-out
   .cmosi(cmosi),                         // Code Memory   Master-out slave-in
   .cmiso(cmiso),                         // Code Memory   Master-in slave-out
   .imosi(imosi),                         // I/O  Register Master-out slave-in
   .imiso(imiso),                         // I/O  Register Master-in slave-out
`ifdef   A2S_ENABLE_SIMD
   .wmosi(wmosi),                         // SIMD Master-out slave-in
   .wmiso(wmiso),                         // SIMD Master-in slave-out
`endif //A2S_ENABLE_SIMD
`ifdef   A2S_ENABLE_INTERRUPTS
   .intpt(intpt),
`endif //A2S_ENABLE_INTERRUPTS
`ifdef   A2S_ENABLE_CONDITION_CODES
   .ccode(ccode),
`endif //A2S_ENABLE_CONDITION_CODES
   .activ(alive),
   .reset(reset),
   .fstck(fstck),
   .clock(gclock)
   );

wire stall =  1'b0;

preicg iCG (.ECK(gclock), .CK(clock), .E(~stall), .SE(1'b0));

// Code Memory ------------------------------------------------------------------------------------------------------------------
// Read-only Port
reg   [WA2S-1:0]  CMo;           // Data Out
wire  [WA2S-1:0]  adCM;          // Address In
wire              enCM;          // Enable Read Only
// Write-only Port
wire  [WA2S-1:0]  adMC, inMC;    // Address and Data In
wire              enMC, wrMC;    // Write Enable
// Decollate
assign   {enCM,          adCM} = cmosi;
assign   {enMC,wrMC,inMC,adMC} = bmosi;

wire  eCM = enMC | enCM;

RAM #(
   .DEPTH      (DCMW),
   .WIDTH      (WA2S)
   ) CM (
   .RAMo  (CMo),
   .iRAM  (inMC),
   .aRAM  (  enMC ? adMC[MACM:2] : adCM[MACM:2]),
   .eRAMn (~eCM ),
   .wRAMn (~wrMC),
   .clock (clock)
   );

reg fred;
always @(posedge clock)
   if (reset)  fred <= 1'b0;
   else        fred <= eCM & ~fred;

reg  CMk;
always @(posedge clock)
   if (reset)  CMk <= 1'b0;
   else        CMk <= enCM;

assign   cmiso = {1'b1, CMk, CMo};

// Data Memory ------------------------------------------------------------------------------------------------------------------
// Read \ Write Port
reg   [WA2S-1:0]  DMo;           // Data Out
wire  [WA2S-1:0]  aDM, iDM;      // Address and Data In
wire              eDM, rDM, wDM; // Enable  Read and Write
// Memory State variables
reg               DMk, DMg;
reg               XC;

assign   {eDM,wDM,rDM,iDM,aDM} = dmosi;

always @(posedge clock)
   XC <= `tCQ eDM & wDM & rDM;

RAM #(
   .DEPTH      (DDMW),
   .WIDTH      (WA2S)
   ) DM (
   .RAMo  (DMo),
   .iRAM  (iDM),
   .aRAM  (aDM[MADM:2]),
   .eRAMn (~(eDM | XC)),
   .wRAMn (~(wDM & ~rDM | XC)),
   .clock (clock)
   );

always @(posedge clock)
   if (reset)  DMk <= 1'b0;
   else        DMk <= eDM & DMg;

always @(posedge clock)
   if (reset)  DMg <= 1'b1;
   else        DMg <= ~( eDM & wDM & rDM );

assign   dmiso =  { DMg, DMk, DMo };

// Work Memory ------------------------------------------------
`ifdef   A2S_ENABLE_SIMD
reg   [127:0]   WMo;
reg             WMk;
wire            WMg;
reg   [127:0]   WM   [0:511];
wire  [127:0]  iWM, mWM;
wire  [ 31:0]  aWM;
wire           eWM, wWM;
integer  i;
assign {eWM, wWM, iWM[127:0], mWM[127:0], aWM[31:0]} = wmosi;

always @(posedge clock)
   if (eWM)
      if (wWM) begin
         for (i=0; i<128; i=i+1)
            if (mWM[i])  WM[aWM[15:4]][i] <= `tCQ iWM[i];
         WMo <= `tCQ WM[aWM[15:4]];
         end
      else
         WMo <= `tCQ WM[aWM[15:4]];

always @(posedge clock)
   WMk <= `tCQ eWM & ~wWM;

assign   wmiso = {1'b1, WMk, WMo};
`endif //A2S_ENABLE_SIMD
// End Work Memory --------------------------------------------

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//      TESTBENCH      //////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

module t;

`include "A2Sv2r3_ISA.vh"
`include "A2Sv2r3_ISAstring.vh"

parameter   DCMB  = 1048576;        // A2S Depth of Code Memory in Bytes
parameter   DDMB  = 1048576;        // A2S Depth of Data Memory in Bytes
parameter   WA2S  = 32;             // A2S Machine Width
parameter   WINS  = 32;             // A2S Instruction Word Width
parameter   WIMM  = 16;             // A2S Immediate Width
parameter   WIOA  = 13;             // A2S IO Address Width
parameter   DRSK  = 32;             // A2S Depth of Return Stack
parameter   DDSK  = 32;             // A2S Depth of Data Stack

// `include "A2Sv2r3_Registers.vh"
// `include "A2Sv2r3_ISA.vh"
// `include "A2Sv2r3_ISAstring.vh"

localparam  initialcode  =  "..//asm//ISAtest_code.rmh";
localparam  initialdata  =  "..//asm//ISAtest_data.rmh";
//localparam  initialcode  =  "test_code.rmh";
//localparam  initialdata  =  "test_data.rmh";
//localparam  initialcode  =  "..//wasm//c2wa2sm//simple//gerald_simple_code.rmh";
//localparam  initialdata  =  "..//wasm//c2wa2sm//simple//gerald_simple_data.rmh";
localparam  com1data     =  "com1data.txt";

localparam  DCMW         =  DCMB / 4;  // A2S Depth of Code Memory in Words
localparam  DDMW         =  DDMB / 4;  // A2S Depth of Data Memory in Words

integer cycle,DScycle,UARTfile;

initial begin
   UARTfile = $fopen("com1uart.txt");
   end

// Debug string for main A2S FSM ------------------------------------------------------------------------------------------------
localparam [4:0] START = 1, VECTOR= 2, FETCH= 4, NEXT = 8;  // FSM States
reg [39:0]  FSMstring;
always @* casez (mut.iA2S.FSM)
   START :  FSMstring  =  "START";
   VECTOR:  FSMstring  =  "VECTR";
   FETCH :  FSMstring  =  "FETCH";
   default  FSMstring  =  "NEXT ";
   endcase

// Debug strings for instruction names in IQ ------------------------------------------------------------------------------------
wire  [31:0]   orderS    = ISAstring(mut.iA2S.FSM[3:0], mut.iA2S.order[5:0], mut.iA2S.fn[15:0], mut.iA2S.YQmt, 1'b0 );

wire  [31:0]   slot0     = ISAstring(mut.iA2S.FSM[3:0], mut.iA2S.I[33:28],   mut.iA2S.fn[15:0], mut.iA2S.YQmt, 1'b1 );
wire  [31:0]   slot1     = ISAstring(mut.iA2S.FSM[3:0], mut.iA2S.I[27:22],   mut.iA2S.fn[15:0], mut.iA2S.YQmt, 1'b1 );
wire  [31:0]   slot2     = ISAstring(mut.iA2S.FSM[3:0], mut.iA2S.I[21:16],   mut.iA2S.fn[15:0], mut.iA2S.YQmt, 1'b1 );
wire  [31:0]   slot3     = ISAstring(mut.iA2S.FSM[3:0], mut.iA2S.I[15:10],   mut.iA2S.fn[15:0], mut.iA2S.YQmt, 1'b1 );
wire  [31:0]   slot4     = ISAstring(mut.iA2S.FSM[3:0], mut.iA2S.I[ 9:4 ],   mut.iA2S.fn[15:0], mut.iA2S.YQmt, 1'b1 );
wire  [ 5:0]   slot4h    =                              mut.iA2S.I[ 9:4 ];
wire  [ 3:0]   slot5     =                              mut.iA2S.I[ 3:0 ];

// Debug strings for depth of literal Queue (YQ) --------------------------------------------------------------------------------
localparam [2:0] MPT   = 0, UNO   = 1, DOS  = 2, TRS  = 3, QTR  = 4;  // YQ state machine
reg [23:0]  YFSMstring;
always @* casez (1'b1)
   mut.iA2S.YFSM[MPT] :  YFSMstring  =  "MPT";
   mut.iA2S.YFSM[UNO] :  YFSMstring  =  "UNO";
   mut.iA2S.YFSM[DOS] :  YFSMstring  =  "DOS";
   mut.iA2S.YFSM[TRS] :  YFSMstring  =  "TRS";
   mut.iA2S.YFSM[QTR] :  YFSMstring  =  "QTR";
   default               YFSMstring  =  "???";
   endcase

// Debug strings for A2S Push and Pop for T0..T3 --------------------------------------------------------------------------------
localparam [3:0]  NOST = 4'd0, P0P1 = 4'd1, P1P0 = 4'd2, P1P1 = 4'd3, P2P0 = 4'd4,
                  P2P1 = 4'd5, P2P2 = 4'd6, P3P1 = 4'd7, P3P3 = 4'd8, P4P4 = 4'd9;
wire  [31:0] pushpopstring =  mut.iA2S.Sm == NOST  ?  "NOST"
                           :  mut.iA2S.Sm == P0P1  ?  "P0P1"
                           :  mut.iA2S.Sm == P1P0  ?  "P1P0"
                           :  mut.iA2S.Sm == P1P1  ?  "P1P1"
                           :  mut.iA2S.Sm == P2P0  ?  "P2P0"
                           :  mut.iA2S.Sm == P2P1  ?  "P2P1"
                           :  mut.iA2S.Sm == P2P2  ?  "P2P2"
                           :  mut.iA2S.Sm == P3P1  ?  "P3P1"
                           :  mut.iA2S.Sm == P3P3  ?  "P3P3"
                           :  mut.iA2S.Sm == P4P4  ?  "P4P4" : "????";

localparam [3:0]  Td0 = 4'b0000,  Td1  = 4'b0001, Td2  = 4'b0011, Td3  = 4'b0111, Td4  = 4'b1111;

wire  [31:0] Tdepth_string =  mut.iA2S.Tdepth == Td1 ? "Tone"
                           :  mut.iA2S.Tdepth == Td2 ? "Ttwo"
                           :  mut.iA2S.Tdepth == Td3 ? "Tthr"
                           :  mut.iA2S.Tdepth == Td4 ? "Tfor"
                           :                           "Tmpt";

// Debug String for main SIMD FSM managing memory pipelining --------------------------------------------------------------------
localparam [2:0]  IDLE = 0, RDRQ = 1, RDBU = 2, RDND = 3, SORQ = 4, MCYC = 5, BUSY = 6;

wire  [63:0] SIMD_FSM_string  = mut.iA2S.i_SIMD.FSM == IDLE ? "Idle    "
                              : mut.iA2S.i_SIMD.FSM == RDRQ ? "Rd Req  "
                              : mut.iA2S.i_SIMD.FSM == RDBU ? "Rd Burst"
                              : mut.iA2S.i_SIMD.FSM == RDND ? "Rd End  "
                              : mut.iA2S.i_SIMD.FSM == SORQ ? "So Req  "
                              : mut.iA2S.i_SIMD.FSM == MCYC ? "MultiCyc"
                              : mut.iA2S.i_SIMD.FSM == BUSY ? "Busy    " : "????    ";


wire  [31:0] Ialigned      =  mut.iA2S.I[33:2];

wire  [39:0]   nI_skew  = {mut.iA2S.nI[32:1],7'h0,mut.iA2S.nI[0]};
wire  [39:0]    I_skew  = {mut.iA2S.I[33:4],mut.iA2S.nI[9:0]};
wire  [ 7:0] DSdipstick = {3'h0, mut.iA2S.iSD.PTR[4:0]};
wire  [ 7:0] RSdipstick = {3'h0, mut.iA2S.iSR.PTR[4:0]};

integer  i, nextcount, fetchcount, FNcheck;

reg  [2*WA2S+1   :0] bmosi;         // Boot Load Code Memory
wire [WIMM+WA2S-1:0] imosi;         // I/O Register Master-out slave-in
wire [WA2S       :0] imiso;         // I/O Register Master-in slave-out
reg                  reset;
reg  [         63:0] CaseIhits, CaseDhits, CaseThits;

reg  [WA2S     -1:0] Scratch;       // Scratch Register
wire [WA2S     -1:0] IOo;           // Data Out from IO Register
wire [WIOA     -1:0] aIO;           // Register Address
wire [WA2S     -1:0] iIO;           // Data Into IO register
wire [          1:0] cIO;           // Control to IO register
wire                 eIO;           // Enable  and Write
reg  [WA2S     -1:0] string4;
reg                  wr_SCR;

reg  [9:0]           ccode;
reg                  gen_clock;
reg                  fastclock;
reg                  quiet;

initial begin
   gen_clock = 1'b0;
   forever
      gen_clock =  #(`A2S_CLOCK_PERIOD/2) ~gen_clock;
   end

initial begin
   fastclock = 1'b0;
   forever
      fastclock = #(`FAST_CLOCK_PERIOD/2) ~fastclock;
   end

wire  clock = gen_clock;

wire  [31:0]   LFSR;
reg            enlfsr;
A2_lfsr #(32) iLFSR (
   .LFSRo   (LFSR),
   .enable  (enlfsr),
   .reset   (reset),
   .clock   (clock)
   );

A2S_v2_wRAMs  #(
   .DCMB  (DCMB),                   // A2S Depth of Code Memory
   .DDMB  (DDMB),                   // A2S Depth of Data Memory
   .WA2S  (WA2S),                   // A2S Machine Width
   .WINS  (WINS),                   // A2S Instruction Word Width
   .DRSK  (DRSK),                   // A2S Depth of Return Stack
   .DDSK  (DDSK),                   // A2S Depth of Data Stack
   .WIMM  (WIMM)                    // A2S Width of Immediates
   ) mut (
   .imosi  (imosi),                 // I/O Register Master-out slave-in
   .imiso  (imiso),                 // I/O Register Master-in slave-out
   .bmosi  (bmosi),                 // Boot Load Code Memory Interface
`ifdef   A2S_ENABLE_INTERRUPTS
   .intpt  (intpt),
`endif
`ifdef   A2S_ENABLE_CONDITION_CODES
   .ccode  (ccode),
`endif
   .reset  (reset),
   .fstck  (fastclock),
   .clock  (clock)
   );

assign   {eIO,cIO,iIO,aIO} = imosi;

// Scratch Register -------------------------------------------------------------------------------------------------------------

wire  rc_SCR  =  eIO & (cIO == 2'b00) & (aIO == `aSCR),
      rd_SCR  =  eIO & (cIO == 2'b01) & (aIO == `aSCR),
      ld_SCR  =  eIO & (cIO == 2'b10) & (aIO == `aSCR),
      sw_SCR  =  eIO & (cIO == 2'b11) & (aIO == `aSCR);

always @(posedge clock) if (ld_SCR || sw_SCR) Scratch <= `tCQ  iIO[WA2S-1:0];
                   else if (rc_SCR)           Scratch <= `tCQ  {WA2S{1'b0}};
                   else if (wr_SCR)           Scratch <= `tCQ  string4;

wire [31:0] IOo_SCR  =  rd_SCR || rc_SCR || sw_SCR ?  Scratch : {WA2S{1'b0}};


//reg   SCR_change;
//always @(posedge clock) begin
//   SCR_change <= `tCQ (ld_SCR || rc_SCR || sw_SCR );
//   if (SCR_change) begin
//      if (mut.iA2S.Tdepth == 0)
//         $display($time," Scratch State = %h \"%s\" T0 = -EMPTY-",Scratch,Scratch);
//      else
//         $display($time," Scratch State = %h \"%s\" T0 = %h",Scratch,Scratch,mut.iA2S.T0);
//      end
//   end

always @(posedge clock)
   if (!quiet && !reset) begin
      if (cycle%100 == 0) begin
         $display("_________________________________________________________________________________________________________________________________________");
         $display("  cycle STATE O_______ P_______ Opcode  T0______ T1______ T2______ T3______ Dstk_top Rstk_top A_______ B_______ C_______ DataMem  SCRATCH");
         end
      $display(" %6d %S %h %h %h %s %h %h %h %h %h %h %h %h %h %h %s",
            cycle,
            FSMstring,
            mut.iA2S.O,
            mut.iA2S.P,
            FSMstring != "NEXT " ? 8'hxx : mut.iA2S.order,
            slot0,
            mut.iA2S.T0,
            mut.iA2S.T1,
            mut.iA2S.T2,
            mut.iA2S.T3,
            mut.iA2S.DSo,
            mut.iA2S.RSo,
            mut.iA2S.A,
            mut.iA2S.B,
            mut.iA2S.C,
            mut.iA2S.DMo,
            Scratch
            );
      end

reg   int_enable;
always @(posedge clock)
   if (reset)                    int_enable <= 1'b0;
   else if  (Scratch == "INTP")  int_enable <= 1'b1;

wire  [31:0]   modulo = LFSR % 23;
assign         intpt  = int_enable & (modulo == 0);

// COM1 Register ----------------------------------------------------------------------------------------------------------------

reg   [7:0] COM1;

wire  rc_COM1 =  eIO & (cIO==2'b00) & (aIO == `aCOM1);
wire  rd_COM1 =  eIO & (cIO==2'b01) & (aIO == `aCOM1);
wire  ld_COM1 =  eIO & (cIO==2'b10) & (aIO == `aCOM1);
wire  sw_COM1 =  eIO & (cIO==2'b11) & (aIO == `aCOM1);

always @(posedge clock)
   if (ld_COM1) begin
      COM1  <= `tCQ  iIO[31:24];
      $fwrite(UARTfile,"%s ",iIO[31:24]);
      end

wire [31:0] IOo_COM1 =  rd_COM1 || rc_COM1 || sw_COM1  ? {COM1,24'h0} : 32'h0;

//reg   COM1_change;
//always @(posedge clock) begin
//   COM1_change <= `tCQ (ld_COM1
//                     || rc_COM1
//                     || sw_COM1 );
//   if (SCR_change) begin
//      if (COM1 == 8'h0) $display(" <- COM1");
//      else              $write  ("%s",COM1);
//      end
//   end
// ------------------------------------------------------------------------------------------------------------------------------

assign   IOo   =  IOo_SCR | IOo_COM1;
assign   imiso =  {eIO,IOo};        // No IOk   as yet feed eIO for zero wait states!

always @(posedge clock)
   if (reset) begin
      fetchcount  <= 0;
      nextcount   <= 0;
      cycle       <= 0;
      DScycle     <= 0;
      end
   else begin casez (mut.iA2S.FSM)
      0,1   : fetchcount <= fetchcount +1;
      2,3   : nextcount  <= nextcount  +1;
      endcase
      cycle <= cycle +1;
      if (!mut.iA2S.iSD.eRF) DScycle <= DScycle +1;
      end

initial begin
   nextcount  = 0;
   fetchcount = 0;
   FNcheck    = 0;
   CaseIhits  = {64{1'b0}};
   CaseDhits  = {64{1'b0}};
   CaseThits  = {64{1'b0}};
   end

always @(posedge clock) begin
   CaseIhits[mut.iA2S.caseI] <= 1'b1;
   CaseDhits[mut.iA2S.caseD] <= 1'b1;
   CaseThits[mut.iA2S.caseT] <= 1'b1;
   end

wire  CaseT19 =  mut.iA2S.caseT == 19;

reg   hc; always @(posedge clock) hc <= reset ? 1'b0 : ~hc;

reg  [31:0] bootaddr;

wire [31:0] A2S_YQo        = mut.iA2S.YQo[32:1];
wire [31:0] A2S_incoming   = mut.iA2S.incoming[32:1];
wire [31:0] A2S_ifromYQ    = mut.iA2S.ifromYQ[32:1];

// Main Test --------------------------------------------------------------------------------------------------------------------
reg   [31:0]   shadow [0:DCMW-1];
reg   [31:0]   umbra  [0:DDMW-1];
reg            test;

`define  FAIL  32'h4641494c
`define  PASS  32'h50415353
`define  BRKP  32'h42524B50
`define  FN    32'h0000464e

initial begin
   quiet    = 1;
   $display(" A2S v2r3 Simulator \n");
   $display(" Code Memory %d bytes",  DCMB);
   $display(" Data Memory %d bytes\n",DDMB);
//   $display(" Clock Rate  %e GHz",(1/(`A2S_CLOCK_PERIOD));
   $readmemh(initialcode,shadow);
   $readmemh(initialdata,umbra );
   $readmemh(initialdata,mut.DM.RAM);
   $display(" Files loaded");
   wr_SCR   = 0;
   reset    = 0;
   ccode    = 0;
   enlfsr   = 1;
   repeat (100) @(posedge clock) #(`A2S_CLOCK_PERIOD/10);
   $display(" clock 100");
   bmosi    = {2*WA2S+2{1'b0}};
   repeat ( 10) @(posedge clock); #1; // (`A2S_CLOCK_PERIOD/10);
   $display(" Reset -- ON ");
   reset    = 1;                             // Hold A2S in reset while we boot load
   quiet    = 0;
   wrScratch("RSET");
   repeat ( 10) @(posedge clock) #(`A2S_CLOCK_PERIOD/10);
   bootaddr = 0;
   repeat ( 10) @(posedge clock) #(`A2S_CLOCK_PERIOD/10);
   $display(" Upload Code to CM");
   wrScratch("upCM");
   while (bootaddr < 16384) begin
      bmosi    = {2'b11,shadow[bootaddr[31:2]],bootaddr};
      bootaddr = bootaddr +4;
      @(posedge clock) `tCQ;
      end
   bmosi    = {2*WA2S+3{1'b0}};
   repeat ( 10) @(posedge clock) #(`A2S_CLOCK_PERIOD/10);
   $display(" Verify Code Upload");
   wrScratch("ckCM");
   for(i=0; i<4096; i=i+1)
      if (shadow[i] !== mut.CM.RAM[i]) begin
         $display(" CM error address %h = %h, should be %h",i,mut.CM.RAM[i],shadow[i]);
         test = test +1;
         end
   if (test > 0) begin
      $display(" Code Test aborted");
      $stop;
      $finish;
      end
   $display(" Code test done \n");
   for(i=0; i<64; i=i+1)
      $display(" CM code top : address  %h = %h ",i,mut.CM.RAM[i]);

   wrScratch("CMok");
   $display(" Verify Data Upload");
   test = 0;
   for(i=0; i<DDMW; i=i+1)
      if (umbra[i] !== mut.DM.RAM[i]) begin
         $display(" DM error: address %h = %h, should be %h",i,mut.DM.RAM [i],umbra[i]);
         test = test +1;
         end
      if (test > 0) begin
         $display(" Data Test aborted");
         $stop;
         $finish;
         end
   $display(" Data test done");
   $display(" Starting test run ");
   $display(" MNQ : %b%b%b",mut.iA2S.M,mut.iA2S.N,mut.iA2S.Q);
   wrScratch("DMok");

   repeat ( 10) @(posedge clock) `tCQ;

   $readmemh(initialdata,mut.DM.RAM);
   reset    = 0;                             // Then let it go!
   $display(" reset off");
   wrScratch("RUN ");

   while ((Scratch !== `FAIL) && (Scratch !== `PASS) && (Scratch !== `BRKP)) @(posedge clock) `tCQ;

   repeat (10) @(posedge clock) `tCQ;

   $display("\n Test Complete : %s",  Scratch);
   $display("\n Cycles Taken  : %d",  cycle  );
   $display("\n DScycles Taken: %d",  DScycle);
   $display("\n MNQ           : %b%b%b",mut.iA2S.M,mut.iA2S.N,mut.iA2S.Q);
   $display("\n\n Coverage    :");
   $display("              44444444 33333333 33222222 22221111 111111           ");
   $display("              76543210 98765432 10987654 32109876 54321098 76543210");
   $display(" CaseI hits = %b_%b_%b_%b_%b_%b",CaseIhits[47:40],CaseIhits[39:32],CaseIhits[31:24],CaseIhits[23:16],CaseIhits[15:8],CaseIhits[7:0]);
   $display(" CaseD hits = %b_%b_%b_%b_%b_%b",CaseDhits[47:40],CaseDhits[39:32],CaseDhits[31:24],CaseDhits[23:16],CaseDhits[15:8],CaseDhits[7:0]);
   $display(" CaseT hits = %b_%b_%b_%b_%b_%b",CaseThits[47:40],CaseThits[39:32],CaseThits[31:24],CaseThits[23:16],CaseThits[15:8],CaseThits[7:0]);

   $fclose(UARTfile);
   $finish;
   end

task wrScratch;
input [31:0] data;
begin
string4 = data;
@(posedge clock) #(`A2S_CLOCK_PERIOD/10)
wr_SCR = 1'b1;
@(posedge clock) #(`A2S_CLOCK_PERIOD/10)
wr_SCR = 1'b0;
end
endtask

endmodule

`endif
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
`ifdef STUFF
`endif
