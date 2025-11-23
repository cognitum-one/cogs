//==============================================================================================================================
//
//   Copyright © 1996..2025 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : 2 port RAM interface B- side with passthrough data
//
//===============================================================================================================================
//      THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//===============================================================================================================================

`ifdef  synthesis //JLPL
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

`define AMOSI_WIDTH 97 //***+++
`define AMISO_WiDTH 97 //***+++

module   RAM_3PX #(
   parameter   DEPTH    =  16384,
   parameter   WIDTH    =  32,                  // Double wide on B side
   parameter   BASE_A   =  32'h40000000,        // Base Address
   parameter   BASE_B   =  32'h40000000,        // Base Address
   parameter   BASE_C   =  32'h40000000,        // Base Address
   parameter   PIPE_C   =  0,                   // Pipe Interface in B side output
   parameter   WR_THRU  =  0,                   // Write Through   0: read OLD write new   1: write new and read new
   parameter   tACC     =  1,                   // Access time in clock cycles     -- supports up to 7 wait states
   parameter   tCYC     =  1                    // RAM cycle time in clock cycles
   )(
   output wire [`DMISO_WIDTH -1:0]  amiso,      // RAM A Port    34 = {agnt, aack, ardd[31:0]} -- grant, acknowledge, 32-bit data
   output wire [`DMISO_WIDTH -1:0]  bmiso,      // RAM B Port    34 = {agnt, aack, ardd[31:0]} -- grant, acknowledge, 32-bit data
   output wire [`AMISO_WIDTH -1:0]  cmiso,      // RAM C Port    97 = {aordy,acmd[7:0],pass[23:0],ard0[31:0],ard1[31:0]}
   input  wire                      cmiso_fb,   //                     apop

   input  wire [`DMOSI_WIDTH -1:0]  amosi,      // RAM A Port :  67 = {areq, awr, ard,  awrd[31:0], aadr[31:0]} //JLPL
   input  wire [`DMOSI_WIDTH -2:0]  bmosi,      // RAM B Port :  66 = {areq, awr,  awrd[31:0], aadr[31:0]} //JLPL
   input  wire [`AMOSI_WIDTH -1:0]  cmosi,      // RAM C Port :  97 = {areq, pass[31:0],wrd[31:0],adr[31:0]}
   input  wire                      cmosi_fb,   // acknowledge has been accepted. Force to 1'b1 if unused
   input  wire                      reset,
   input  wire                      clock
   );

localparam     AMSB  = `LG2(DEPTH * (WIDTH/8)) -1,    // Most significant Address bit
               ALSB  = `LG2(WIDTH/8),                 // Least significant Address bit
               LG2D  = `LG2(DEPTH/8);                 // e.g. 4096 -> 12

// Nets -------------------------------------------------------------------------------------------------------------------------
reg   [ 4:0]   SSM;                             // Selection State Machine //JLSW
reg   [ 7:0]   WSM;                             // Wait State Machine
reg            AOK, BOK, COK;                   // Acknowledge 'hold' flops until 'aok' accepts it
reg   [31:0]   PASS;                            // Passthrough for command/acknowldege and tag fields

wire  [63:0]   RAMo, iRAM, mRAM; //JL

wire  [LG2D-1:0]  aRAM;
wire              eRAM, wRAM;

wire  [31:0]   awrd, bwrd, cwrd;
wire  [31:0]   aadr, badr, cadr;
wire           areq, breq, creq;
wire           agnt, bgnt, cgnt;
wire           awr,  bwr,  cwr;
wire           aack, back, cack;
wire           asel, bsel, csel;
wire  [23:0]               cpas;
wire  [ 7:0]               ccmd;
wire           reqA, reqB, reqC;
wire           mreq, mack;

wire           cacpt; //JL
reg            Bank; //JL
wire  [95:0]   pipei; //JL

// Decollate buses --------------------------------------------------------------------------------------------------------------
wire dummya;

assign   {areq,awr,dummya,            awrd[31:0],aadr[31:0]}  =  amosi[66:0],        // 1+1+1+32+32    =   67 //JLPL
         {breq,bwr,                   bwrd[31:0],badr[31:0]}  =  bmosi[66:0],        // 1+1+32+32    =   66
         {ccmd[7:0],cpas[23:0],cwrd[31:0],cadr[31:0]}  =  cmosi[96:0];        // 1+8+24+32+32 =   97

// Qualify  Requests
assign   reqA  =  areq     & (aadr[31:30] == BASE_A[31:30]),
         reqB  =  breq     & (badr[31:30] == BASE_B[31:30]),
         reqC  =  ccmd[7]  & (cadr[31:30] == BASE_C[31:30]);

// FSM --------------------------------------------------------------------------------------------------------------------------
localparam  [2:0] IDLE = 0, AACC = 1, BACC = 2, CACC = 3, STAY = 4;
// A has absolute priority over B which has absolute priority over C!!!
always @(posedge clock)
   if (reset)                             SSM <= `tCQ 1'b1 << IDLE;
   else (* parallel_case *) case (1'b1)
      SSM[IDLE]   : if (reqA           )  SSM <= `tCQ 1'b1 << AACC;
               else if (reqB           )  SSM <= `tCQ 1'b1 << BACC;
               else if (reqC           )  SSM <= `tCQ 1'b1 << CACC;
      SSM[AACC]   : if (mack  &&  reqB )  SSM <= `tCQ 1'b1 << BACC;
               else if (mack  &&  reqC )  SSM <= `tCQ 1'b1 << CACC;
               else if (mack  && !reqA )  SSM <= `tCQ 1'b1 << IDLE;
      SSM[BACC]   : if (mack  &&  reqA )  SSM <= `tCQ 1'b1 << AACC;
               else if (mack  &&  reqC )  SSM <= `tCQ 1'b1 << CACC;
               else if (mack  && !reqB )  SSM <= `tCQ 1'b1 << IDLE;
      SSM[CACC]   : if (mack  && !cacpt)  SSM <= `tCQ 1'b1 << STAY;  // Channel C must wait for its Ack to be accepted
               else if (mack  &&  reqA )  SSM <= `tCQ 1'b1 << AACC;
               else if (mack  &&  reqB )  SSM <= `tCQ 1'b1 << BACC;
               else if (mack           )  SSM <= `tCQ 1'b1 << IDLE;
      SSM[STAY]   : if (reqA  &&  cacpt)  SSM <= `tCQ 1'b1 << AACC;
               else if (reqB  &&  cacpt)  SSM <= `tCQ 1'b1 << BACC;
               else if (          cacpt)  SSM <= `tCQ 1'b1 << IDLE;
      endcase

assign   agnt  =  SSM[IDLE] | ~SSM[IDLE] & mack,
         bgnt  = ~reqA &  agnt,
         asel  =  reqA &  agnt,
         bsel  =  reqB &  bgnt,
         csel  =  reqC &  cgnt,
         crnc  =  csel & (ccmd[4:3] == 2'b00),     // Read and clear -- read old contents and write zeros
         mreq  =  asel |  bsel;

// Multiplex between buses ------------------------------------------------------------------------------------------------------
assign   eRAM  =  asel |  bsel,
         aRAM  = {LG2D{asel}}  & aadr[AMSB:ALSB]
               | {LG2D{bsel}}  & badr[AMSB:ALSB],
         bank  =  aadr[ALSB-1],
         iRAM  =   {64{asel}}  & {2{awrd[31:0]}}
               |   {64{bsel}}  & {2{bwrd[31:0]}}
               |   {64{crnc}}  &  64'h0000000000000000
               |   {64{csel}}  & {2{cwrd[31:0]}}, //JL
         mRAM  =  aadr[ALSB-1] ?  64'hffffffff00000000 : 64'h00000000ffffffff,
         wRAM  =       asel    &  awr
               |       bsel    &  bwr
               |       csel    & (ccmd[4:3] != 2'b01);   // Only 'read' doesn't write!

// RAM --------------------------------------------------------------------------------------------------------------------------
// ARM High Density Single Port SRAM RVT MVT Compiler
//    12LP+ Process ROM 0.064 um^2 Bit Cell
//    Number of Words            2048
//    Number of Bits             64
//    Multiplexer width          4
//    Number of Banks            2
//    Pipeline                   off
//    Bit-Write Mask             off
//    Write-thru                 off
//    Top Metal Layer            m5-m10
//    Power type                 otc
//    Redundancy                 off
//    Scan Only                  off
//    Soft Error Repair          none
//    Power gating               on
//    Back Biasing               off
//    Retention                  on
//    Extra Margin Adjustment    on
//    Advnaced Test Features     off
//    Write assist               on
//    End of life guardband      0
//    LeftRight Bank Enable      off

mRAM #(DEPTH/8,2*WIDTH,`LG2(DEPTH/8)) i_MaskRAM (  // READ BEFORE WRITE  << NO WRITE THRU >> //JLSW
   .RAMo    (RAMo[2*WIDTH-1:0]),    // Data Output /* MULTI-CYCLE NET */
   .iRAM    (iRAM[2*WIDTH-1:0]),    // Data Input
   .mRAM    (mRAM[2*WIDTH-1:0]),    // Write Mask per word
   .aRAM    (aRAM[   LG2D-1:0]),    // Address
   .eRAM    (eRAM             ),    // Enable
   .wRAM    (wRAM             ),    // Write
   .clock   (clock            )
   );

always @(posedge clock)
   if (csel)   PASS <= `tCQ  {ccmd[7:0],cpas[23:0]};

// Wait State Machine ----------------------------------------------------------------------------------------------------------
localparam  [2:0]  WAIT = 0,  LAST = 7;

always @(posedge clock)
   if (reset)                 begin WSM <= `tCQ 1'b1 << IDLE;        Bank <= `tCQ bank; end
   else case(1'b1)
      WSM[WAIT] : if (mreq)   begin WSM <= `tCQ 1'b1 << (8 - tCYC);  Bank <= `tCQ bank; end
      WSM[LAST] : if (mreq)         WSM <= `tCQ 1'b1 << (8 - tCYC);
                  else              WSM <= `tCQ 1'b1 << WAIT;
      default                       WSM <= `tCQ WSM  << 1'b1;
      endcase

assign
   mack  =  WSM[8-tACC] |  SSM[WAIT],
   aack  =  mack        &  SSM[AACC],
   back  =  mack        &  SSM[BACC], //JL
   cack  =  mack        &  SSM[CACC];

// Recollate buses for output --------------------------------------------------------------------------------------------------

assign
   amiso =  {agnt, aack, {32{aack}} & (Bank ? RAMo[63:32] : RAMo[31:0])},
   bmiso =  {bgnt, back, {32{back}} & (Bank ? RAMo[63:32] : RAMo[31:0])},
   pipei =  {!cack,PASS[30:16], PASS[7:0], PASS[15:8], RAMo[31:0], RAMo[63:32]}; // NB:  swapped source and destination tags

A2_pipe #(96) i_Cppl (
   // Output interface
   .por  (cmosi[96]     ),   // ordy  out
   .pdo  (cmosi[95:0]   ),   // data  out
   .prd  (cmosi_fb      ),   // read  in
   // Inputs
   .pir  (cacpt         ),   // irdy  out  B-side accept
   .pdi  (pipei[95:0]   ),   // data  in
   .pwr  (cack          ),   // write in
   .clock(clock         ),
   .reset(reset         )
   );

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
