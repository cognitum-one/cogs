/* ******************************************************************************************************************************

   Copyright © 2022..2024 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         :  A2S Stack Machine -- variant 2 revision 3

   Description          :  Single precision Floating Point, and Integer Multiply

*********************************************************************************************************************************

   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

********************************************************************************************************************************/

module A2Sv2r3_SPFPU #(
   parameter BASE       =  `SPFPU_base
   )(
   // imiso/imosi interface
   output wire [ 32:0]  imiso,      // I/O data out to  A2S  33 {IOk, IOo[31:0] }
   input  wire [ 47:0]  imosi,      // I/O data in from A2S  48 {cIO[2:0] aIO[12:0], iIO[31:0] }
   // A2S FPU inteface outputs
   output wire [ 98:0]  fmiso,      // FPU Port back to A2S  99 {FPit, FPex, FPir,   Fo[31:0], Mo[63:0] }
   input  wire [106:0]  fmosi,      // FPU Port from    A2S 107 {eFP,  imul, istall, cFP[7:0], F2[31:0], F1[31:0], F0[31:0] }
   // Globals
   input  wire          reset,
   input  wire          clock
   );

`ifdef RELATIVE_FILENAMES
`include "A2Sv2r3_ISA.vh"
`else
`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2Sv2r3_ISA.vh"
`endif

// Locals =======================================================================================================================

localparam  x = 1'bx, l = 1'b0, h = 1'b1;
// Input Pipeline Registers
reg            p1_imFPM;
reg   [31:0]   p1_iaFPM, p1_ibFPM,  p1_icFPM;
reg   [ 2:0]   p1_fnFPM;
reg   [ 2:0]   p1_rmFPM;

reg   [31:0]   p1_iaFPV;
reg   [ 1:0]   p1_fnFPV;
reg   [ 2:0]   p1_rmFPV;

reg   [31:0]   p1_iaFPC, p1_ibFPC;
reg   [ 4:0]   p1_fnFPC;

reg   [ 7:0]   p1_cFP;

// Finite State Machin
reg   [ 7:0]   FPUfsm;
// Floating Point Control/Status Register
reg   [31:0]   FCS;
// Conversions
wire  [31:0]   FPVo;
wire           FPVif, FPVxf, FPVdf;
// Relationals et al.
wire  [31:0]   FPCo, FPCresult;
wire           FPCif, FPCdf, FPCgt, FPClt, FPCuo;
// Arithmetics
wire  [31:0]   FPMo;
wire           FPMif, FPMdf, FPMvf, FPMuf, FPMxf;
// Second stage
reg   [31:0]   p2_FPMo;    // (* SiART : Clone and Retime *)

wire   [2:0]   sFP;        // Output selects
wire   [2:0]   sFC;

wire           FPex;       // Exception -- May cause an interrupt

// Decollate fmosi ==============================================================================================================
wire  [31:0]   F0, F1, F2;             // Input Operands
wire  [ 7:0]   cFP;                    // Command functions
wire           eFP;                    // Enable FPU
wire           imul, istall;
assign
   {eFP, imul, istall, cFP[7:0], F2[31:0], F1[31:0], F0[31:0]} = fmosi[106:0];

// Illegal Opcodes ==============================================================================================================
reg   FPit;
always @* (* parallel_case *) casez (cFP[7:0])
   8'b00???0??, 8'b00???111, 8'b011??0??, 8'b011??111,
   8'b10100001, 8'b1010001?, 8'b1010010?, 8'b10100110,
   8'b10101100, 8'b10101001, 8'b10110001, 8'b11111000,
   8'b11111100, 8'b11111110 :   FPit = 1'b0;
   default                      FPit =  eFP;             // Illegal (Unused) Opcode
   endcase

// Pipeline & FSM ===============================================================================================================

localparam [7:0] IDLE = 0 /* default */, WAIT = 1, FPM_IMU = 2, FPM_3op = 3, FPM_2op = 4, FPV_1op = 5, FPC_2op = 6, FPC_1op = 7;

always @(posedge clock)
   if (reset)                                                                 FPUfsm  <= `tCQ 8'h01 << IDLE;
   else case ( 1'b1 )
      default           if (       !istall &&  imul)                          FPUfsm  <= `tCQ 8'h01 << FPM_IMU;
                   else if (eFP && !istall && !cFP[7] && !cFP[6] && !cFP[5])  FPUfsm  <= `tCQ 8'h01 << FPM_3op;
                   else if (eFP && !istall && !cFP[7] && !cFP[6] &&  cFP[5])  FPUfsm  <= `tCQ 8'h01 << FPM_2op;
                   else if (eFP && !istall && !cFP[7] &&  cFP[6] &&  cFP[5])  FPUfsm  <= `tCQ 8'h01 << FPV_1op;
                   else if (eFP && !istall &&  cFP[7] && !cFP[6] &&  cFP[5])  FPUfsm  <= `tCQ 8'h01 << FPC_2op;
                   else if (eFP && !istall &&  cFP[7] &&  cFP[6] &&  cFP[5])  FPUfsm  <= `tCQ 8'h01 << FPC_1op;
      FPUfsm[FPM_3op] : if (!istall)                                          FPUfsm  <= `tCQ 8'h01 << WAIT;
      FPUfsm[FPM_2op] : if (!istall)                                          FPUfsm  <= `tCQ 8'h01 << WAIT;
      FPUfsm[FPM_IMU] : if (!istall)                                          FPUfsm  <= `tCQ 8'h01 << IDLE;
      FPUfsm[FPV_1op] : if (!istall)                                          FPUfsm  <= `tCQ 8'h01 << IDLE;
      FPUfsm[FPC_2op] : if (!istall)                                          FPUfsm  <= `tCQ 8'h01 << IDLE;
      FPUfsm[FPC_1op] : if (!istall)                                          FPUfsm  <= `tCQ 8'h01 << IDLE;
      FPUfsm[WAIT]    : if (!istall)                                          FPUfsm  <= `tCQ 8'h01 << IDLE;
      endcase

assign
   sFP[2]   =  FPUfsm[WAIT]    | FPUfsm[FPM_2op],   // FPM
   sFP[1]   =  FPUfsm[FPC_2op] | FPUfsm[FPC_1op],   // FPC
   sFP[0]   =  FPUfsm[FPV_1op];                     // FPV

// Stall control
// FMAC unit is a 3 cycle total pipeline from T0 back to T0 for floating point
// and possibly only 2 cycles for Integer Multiply when macro PIPELINE_INTEGER_MULTIPLY is defined
wire  FPir  =   FPUfsm[IDLE] & (eFP | imul) & ~istall | FPUfsm[FPM_3op] | FPUfsm[FPM_2op];

// Separate input pipes per module to minimise power consumption
always @(posedge clock) if (eFP)                                     p1_cFP[7:0]    <= `tCQ  cFP[ 7:0];

always @(posedge clock) if (eFP && !cFP[7] && !cFP[6]  ||  imul  )   p1_iaFPM[31:0] <= `tCQ   F0[31:0];
always @(posedge clock) if (eFP && !cFP[7] && !cFP[6]  ||  imul  )   p1_ibFPM[31:0] <= `tCQ   F1[31:0];
always @(posedge clock) if (eFP && !cFP[7] && !cFP[6]  && !cFP[5])   p1_icFPM[31:0] <= `tCQ   F2[31:0];
always @(posedge clock) if (eFP && !cFP[7] && !cFP[6])               p1_fnFPM[ 2:0] <= `tCQ  cFP[ 5:3];
always @(posedge clock) if (eFP && !cFP[7] && !cFP[6]  ||  imul )    p1_rmFPM[ 2:0] <= `tCQ  cFP[ 2:0];

always @(posedge clock) if (eFP && !cFP[7] &&  cFP[6]  &&  cFP[5])   p1_iaFPV[31:0] <= `tCQ   F0[31:0];
always @(posedge clock) if (eFP && !cFP[7] &&  cFP[6]  &&  cFP[5])   p1_fnFPV[ 1:0] <= `tCQ  cFP[ 4:3];
always @(posedge clock) if (eFP && !cFP[7] &&  cFP[6]  &&  cFP[5])   p1_rmFPV[ 2:0] <= `tCQ  cFP[ 2:0];

always @(posedge clock) if (eFP &&  cFP[7] &&  cFP[5])               p1_fnFPC[ 4:0] <= `tCQ  cFP[ 4:0];
always @(posedge clock) if (eFP &&  cFP[7])                          p1_iaFPC[31:0] <= `tCQ   F0[31:0];
always @(posedge clock) if (eFP &&  cFP[7] && !cFP[6])               p1_ibFPC[31:0] <= `tCQ   F1[31:0];
                   else if (eFP &&  cFP[7] &&  cFP[6])               p1_ibFPC[31:0] <= `tCQ   F0[31:0]; // for NaN

// Floating Point Units =========================================================================================================

// Conversions to and from Floating Point and Signed\Unsigned Integer -----------------------------------------------------------

A2F_v fpv (
   .Fpv_Result_x2 (FPVo ),             // output [31:0]  data result
   .Fpv_IF_x2     (FPVif),             // output         invalid operation flag
   .Fpv_XF_x2     (FPVxf),             // output         inexact flag
   .Fpv_DF_x2     (FPVdf),             // output         denormal flag
   .Fpv_Opcode_x1 (p1_fnFPV[ 1:0]),    // input   [1:0]  operation code (0=f2s, 1=s2f, 2=f2u, 3=u2f)
   .Fpv_Rm_x1     (p1_rmFPV[ 1:0]),    // input   [1:0]  rounding mode
   .Fpv_Src_x1    (p1_iaFPV[31:0])     // T0 input  [31:0]  Source operand
   );

// Floating Point Compare and miscellaneous functions ---------------------------------------------------------------------------

A2F_c fpc (
   .Fpc_Result_x1 (FPCresult),         // output [31:0]  data result
   .Fpc_IF_x1     (FPCif),             // output         invalid operation flag
   .Fpc_DF_x1     (FPCdf),             // output         denormal flag
   .Fpc_LT_x1     (FPClt),             // output         less than flag
   .Fpc_GT_x1     (FPCgt),             // output         greater than flag
   .Fpc_UO_x1     (FPCuo),             // output         unordered flag
   .Fpc_Opcode_x1 (p1_fnFPC[ 4:0]),    // input  [ 4:0]  operation code (0=add, 1=sub)
   .Fpc_SrcA_x1   (p1_iaFPC[31:0]),    // T1 input  [31:0]  A operand                     --  A < B    T1 < T0
   .Fpc_SrcB_x1   (p1_ibFPC[31:0])     // T0 input  [31:0]  flip to A operand for NaN
   );

assign
   sFC[2]   =  p1_fnFPC[3:0] == FCHS[3:0],      // Added Change Sign Op
   sFC[1]   =  p1_fnFPC[4:3] == 2'b00,          // Compare outputs to TRUE/FALSE for A2S
   sFC[0]   = ~sFC[2] & ~sFC[1];                // Default

A2_mux #(3,32) Imux1 (.z(FPCo[31:0]), .s(sFC[2:0]), .d({~p1_iaFPC[31], p1_iaFPC[30:0],  {32{FPCresult[0]}},  FPCresult[31:0]}));

// Floating Point Arithmetic and Integer Multiply operations --------------------------------------------------------------------

wire  [7:0] fd = ~ FPUfsm[FPM_IMU] << p1_fnFPM[2:0];
reg   [2:0] fnFPM;
reg         invB, invC, sB, sC;
wire  [2:0] fnM = {1'b1,p1_rmFPM[1:0]};

always @* if ( FPUfsm[FPM_IMU] )                                                 // integer multiply
`define             FM    { fnFPM,invB,invC,sB,sC}
   //                       |        | |     | |
   //                       |        | |     | |
                   `FM  = { fnM,     l,l,    h,h };
   else (* parallel_case *) case (1'b1)
   //                       |        | |     | |
   fd[FMAD[5:3]]  :`FM  = { 3'b001,  l,l,    h,h };
   fd[FMSB[5:3]]  :`FM  = { 3'b001,  l,h,    h,h };
   fd[FMNA[5:3]]  :`FM  = { 3'b001,  h,l,    h,h };
   fd[FMNS[5:3]]  :`FM  = { 3'b001,  h,h,    h,h };
   fd[FMUL[5:3]]  :`FM  = { 3'b000,  l,l,    h,l };
   fd[FADD[5:3]]  :`FM  = { 3'b001,  l,l,    l,l };
   fd[FSUB[5:3]]  :`FM  = { 3'b001,  h,l,    l,l };
   fd[FRSB[5:3]]  :`FM  = { 3'b001,  l,h,    l,l };
   default         `FM  = { 3'b011,  x,x,    x,x };
   endcase

wire  [63:0]   Mo;
wire  [31:0]
   iaFPM =  p1_iaFPM[31:0],
   ibFPM =  sB ? {invB ^ p1_ibFPM[31], p1_ibFPM[30:0]} : {invB,31'h3f800000},
   icFPM =  sC ? {invC ^ p1_icFPM[31], p1_icFPM[30:0]} : {invC ^ p1_ibFPM[31], p1_ibFPM[30:0]};
wire  [ 1:0]
   rmFPM =  p1_rmFPM[1:0];

A2F_m_wrap fpm (
   // Outputs
   .FPMim   (Mo[63:0]),
   .FPMo    (FPMo[31:0]),
   .FPMif   (FPMif),
   .FPMdf   (FPMdf),
   .FPMvf   (FPMvf),
   .FPMuf   (FPMuf),
   .FPMxf   (FPMxf),
   // Inputs
   .fnFPM   (fnFPM[ 2:0]),
   .rmFPM   (rmFPM[ 1:0]),
   .iaFPM   (iaFPM[31:0]),    // T0 Multiplicand
   .ibFPM   (ibFPM[31:0]),    // T1 Multiplier
   .icFPM   (icFPM[31:0])     // T2 Addend
   );

always @(posedge clock) if (FPUfsm[FPM_2op] || FPUfsm[FPM_3op]) begin     // (* SiART -- RETIME *)
   p2_FPMo  <= `tCQ FPMo;
   end

// Output from FPU ==============================================================================================================

wire  [31:0]   Fo;
A2_mux #(3,32) iMXFP (.z(Fo), .s(sFP[2:0]), .d({p2_FPMo, FPCo, FPVo}));

// Collate
assign   fmiso[ 97:0]   =  { FPit, FPex, FPir, Fo[31:0], Mo[63:0] };

//===============================================================================================================================
// Floating Point Control and Status Register ===================================================================================
//===============================================================================================================================
wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
wire                IOk;
// Decollate imosi
assign   {cIO[2:0], iIO[31:0], aIO[12:0]} = imosi[47:0];

wire              wr = cIO==3'b110;
wire              rd = cIO==3'b101;
wire        in_range = aIO[12:1] == BASE[12:1];
wire  [1:0]   decode = in_range  <<  aIO[0];

wire  ld_FCS  =  wr & decode[`iFCS],
      ld_FRM  =  wr & decode[`iFRM],
      rd_FCS  =  rd & decode[`iFCS],
      rd_FRM  =  rd & decode[`iFRM];

// Collate imiso
assign
   imiso[32:0]     = {IOk, IOo[31:0]};

// Second stage pipeline for flags from FPM module

reg   p2_FMvf, p2_FMuf, p2_FMxf, p2_FMif, p2_FMdf;    // (* SiART -- RETIME *)

always @(posedge clock) if (FPUfsm[FPM_3op] || FPUfsm[FPM_2op]) begin
   p2_FMvf  <= `tCQ FPMvf;
   p2_FMuf  <= `tCQ FPMuf;
   p2_FMxf  <= `tCQ FPMxf;
   p2_FMif  <= `tCQ FPMif;
   p2_FMdf  <= `tCQ FPMdf;
   end

// Floating Point Status Register -----------------------------------------------------------------------------------------------
//
//            3 3 2 2 2 2 2 2 2 2 2 2 1 1 1 1 1 1 1 1 1 1
//            1 0 9 8 7 6 5 4 3 2 1 0 9 8 7 6 5 4 3 2 1 0 9 8 7 6 5 4 3 2 1 0
//           |       |       |       |       |       |       |       |       |
//           +-----------+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//       FCS |   000000  |L|G|U|N|D|X|U|V|Z|I|0|X|D|S|H|0|  RM |N|D|X|U|V|Z|I|
//           +-----------+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//                         \ \ \ \ \ \ \ \ \ \   \ \ \ \    \    \ \ \ \ \ \ \__ IM: Mask Invalid Operation
//                          \ \ \ \ \ \ \ \ \ \   \ \ \ \    \    \ \ \ \ \ \___ ZM: Mask Divide by Zero
//                           \ \ \ \ \ \ \ \ \ \   \ \ \ \    \    \ \ \ \ \____ VM: Mask Overflow
//                            \ \ \ \ \ \ \ \ \ \   \ \ \ \    \    \ \ \ \_____ UM: Mask Underflow
//                             \ \ \ \ \ \ \ \ \ \   \ \ \ \    \    \ \ \______ XM: Mask Inexact
//                              \ \ \ \ \ \ \ \ \ \   \ \ \ \    \    \ \_______ DM: Mask Denormal
//                               \ \ \ \ \ \ \ \ \ \   \ \ \ \    \    \________ NM: Mask Not-a-number
//                                \ \ \ \ \ \ \ \ \ \   \ \ \ \    \
//                                 \ \ \ \ \ \ \ \ \ \   \ \ \ \    \___________ RM: Rounding Mode
//                                  \ \ \ \ \ \ \ \ \ \   \ \ \ \                    000   Round to nearest
//                                   \ \ \ \ \ \ \ \ \ \   \ \ \ \                   001   Round down
//                                    \ \ \ \ \ \ \ \ \ \   \ \ \ \                  010   Round up
//                                     \ \ \ \ \ \ \ \ \ \   \ \ \ \                 011   Truncate
//                                      \ \ \ \ \ \ \ \ \ \   \ \ \ \                100   Nearest
//                                       \ \ \ \ \ \ \ \ \ \   \ \ \ \
//                                        \ \ \ \ \ \ \ \ \ \   \ \ \ \_________ HP: Half Precision
//                                         \ \ \ \ \ \ \ \ \ \   \ \ \__________ SP: Single Precision
//                                          \ \ \ \ \ \ \ \ \ \   \ \___________ DP: Double Precision
//                                           \ \ \ \ \ \ \ \ \ \   \____________ XP: Extended Precision
//                                            \ \ \ \ \ \ \ \ \ \
//                                             \ \ \ \ \ \ \ \ \ \______________ IF: Flag Invalid Operation
//                                              \ \ \ \ \ \ \ \ \_______________ ZF: Flag Divide by Zero
//                                               \ \ \ \ \ \ \ \________________ VF: Flag Overflow
//                                                \ \ \ \ \ \ \_________________ UF: Flag Underflow
//                                                 \ \ \ \ \ \__________________ XF: Flag Inexact
//                                                  \ \ \ \ \___________________ DF: Flag Denormal
//                                                   \ \ \ \____________________ NF: Flag Not-a-number
//                                                    \ \ \
//                                                     \ \ \____________________ UO: Compare result is Unordered
//                                                      \ \_____________________ GT: Compare result is Greater Than
//                                                       \______________________ LT: Compare result is Less Then
//
localparam  // Names for bits in FCS
   LT = 25, GT = 24, UO = 23,
   NF = 22, DF = 21, XF = 20, UF = 19, VF = 18, ZF = 17, IF = 16,
   XP = 14, DP = 13, SP = 12, HP = 11,
   RM2=  9, RM0=  7,
   NM =  6, DM =  5, XM =  4, UM =  3, VM =  2, ZM =  1, IM =  0;

always @(posedge clock)
   if (reset)
      FCS[31:0]  <= `tCQ  32'h 0000_107f;                // Set to single precision for this incarnation and all masks
   else if (ld_FCS) begin
      FCS[25:16] <= `tCQ  iIO[25:16];                    // Precision bits are read-only
      FCS[10:0]  <= `tCQ  iIO[10:0];
      end
   else if (ld_FRM)
      FCS[RM2:RM0]  <= `tCQ  iIO[2:0];                   // Load RM bits seperately
   else begin
      // Fpm
      if (FPUfsm[FPM_3op] | FPUfsm[FPM_2op]) begin
         FCS[IF]  <= `tCQ  FCS[IF] | p2_FMif;            // Flags from MAC unit
         FCS[VF]  <= `tCQ  FCS[VF] | p2_FMvf;
         FCS[UF]  <= `tCQ  FCS[UF] | p2_FMuf;
         FCS[XF]  <= `tCQ  FCS[XF] | p2_FMxf;
         FCS[DF]  <= `tCQ  FCS[DF] | p2_FMdf;
         end
      // Fpc
      if (FPUfsm[FPC_2op]) begin
         FCS[LT]  <= `tCQ  FCS[LT] | FPClt;               // Compare results  =,<>,>,>=,<,<=, min & max only
         FCS[GT]  <= `tCQ  FCS[GT] | FPCgt;
         FCS[UO]  <= `tCQ  FCS[UO] | FPCuo;
         end
      if (FPUfsm[FPC_2op] || FPUfsm[FPC_1op])
         FCS[IF]  <= `tCQ  FCS[IF] | FPCif;               // Flags from Compare unit
      if (FPUfsm[FPC_2op] || FPUfsm[FPC_1op])
         FCS[DF]  <= `tCQ  FCS[DF] | FPCdf;
      // Fpv
      if (FPUfsm[FPV_1op])                                // FLags from Conversion unit
         FCS[XF]  <= `tCQ  FCS[XF] | FPVxf;
      if (FPUfsm[FPV_1op] && p1_fnFPV[0]) begin
         FCS[IF]  <= `tCQ  FCS[IF] | FPVif;
         FCS[DF]  <= `tCQ  FCS[DF] | FPVdf;
         end
      end

// FCS Outputs ------------------------------------------------------------------------------------------------------------------
assign   FPex        =  |(~FCS[DM:IM] & FCS[DF:IF]);

assign   IOo         =  {32{rd_FCS}} &        FCS[31:0]
                     |  {32{rd_FRM}} & {29'b0,FCS[RM2:RM0]};

assign   IOk         =   cIO[2] & in_range;

endmodule
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
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
