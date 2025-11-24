/* ******************************************************************************************************************************

   Copyright © 2021 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : Edradour

   Description          : Tile Output -- A2S creates access to the rest of the chip

*********************************************************************************************************************************
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*********************************************************************************************************************************

//**** Put Comments for this PROTOTYPE example
//PROTOTYPE:
//
//A2S_extern_CoP #(`aGATEWY_base) i_GW (
//   // Request Out
//   .miso       (),            // ---->
//   .ordy       (),            // --->\/
//   .pop        (),            // <---/\
//   // Response In
//   .mosi       (),            // <----
//   .push       (),            // <---\/
//   .irdy       (),            // --->/\
//   // Setup
//   .imiso      (),
//   .imosi      (),
//   .intpt_fini (),
//   .intpt_timo (),
//   .timeout    (),
//   .myID       (),
//   // Global
//   .reset      (),
//   .clock      (k)
//   );

********************************************************************************************************************************/

`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_defines.vh"

module   A2_gateway_CoP #(
   parameter   [12:0]         BASE = 13'h00000,  // Base Address of Co-processor
   parameter              NUM_REGS = 12
   )(
   // Request out to Raceway
   output wire [`XMISO_WIDTH-3:0]   miso,       // Tile Outgoing to CDCo
   output wire                      ordy,       // CoP is ready to send
   input  wire                      pop,        // CDC takes bmiso
   // Response in from Raceway
   input  wire [`XMOSI_WIDTH-3:0]   mosi,       // Tile Response Incoming from iCDC
   input  wire                      push,       // CDC returns Response
   output wire                      irdy,       // CoP is ready to receive
   // A2S IO Interface
   output wire [`IMISO_WIDTH-1:0]   imiso,      // Slave data out {                IOk, IOo[31:0]}
   input  wire [`IMOSI_WIDTH-1:0]   imosi,      // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
   output wire                      intpt_fini, // Access to Raceway Finished -- Response received
   output wire                      intpt_timo, // Timed-out
   input  wire [             7:0]   myID,
   // Global
   output wire                      flag,       // flag A2S
   input  wire                      reset,
   input  wire                      clock
   );

`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_IO_addresses.vh"

// Physical Registers ===========================================================================================================

reg   [31:0]   CMND, ADDR, DATA,
               RDA1, RDA0, RESP,
               TIMB, TIMR, FLAG;
reg            BUSY, TIMO;

// Interface decoding -----------------------------------------------------------------------------------------------------------

wire  [31:0]   iIO;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
wire           time0;

// Decollate imosi
localparam  LNRG  =  `LG2(NUM_REGS);

assign   {cIO[2:0], iIO[31:0], aIO[12:0]} = imosi;
wire                       wr = cIO==3'b110;
wire                       rd = cIO==3'b101;
wire                       sw = cIO==3'b111;       // swap
wire                       rs = rd | sw;
wire                       ws = wr | sw;


wire                 in_range = aIO[12:LNRG] == BASE[12:LNRG];
wire  [NUM_REGS-1:0] decode   = in_range << aIO[LNRG-1:0];
wire  [NUM_REGS-1:0]  write   = {NUM_REGS{wr}} & decode;
wire  [NUM_REGS-1:0]   read   = {NUM_REGS{rd}} & decode;

// Writes
wire  ld_ADDR  = write[GATEWAY_ADDR],   // Push Address
      ld_DATA  = write[GATEWAY_DATA],   // Push Data
      ld_CMND  = write[GATEWAY_CMND],   // Push Command -- kicks off external access
      ld_RDA1  = write[GATEWAY_RDA1],   // Read Data 1
      ld_RDA0  = write[GATEWAY_RDA0],   // Read Data 0
      ld_RESP  = write[GATEWAY_RESP],   // Response
      ld_TIMO  = write[GATEWAY_TIMO],   // TimeOut
      ld_TIMB  = write[GATEWAY_TIMB],   // TimeBase
      ld_TIMR  = write[GATEWAY_TIMR],   // Timer
      ld_FLAG  = write[GATEWAY_FLAG];   // Flag
// Reads
wire  rd_MyID  =  read[GATEWAY_MyID],   // Read Tile ID
      rd_ADDR  =  read[GATEWAY_ADDR],   // Pop  Address
      rd_DATA  =  read[GATEWAY_DATA],   // Pop  Data
      rd_CMND  =  read[GATEWAY_CMND],   // Pop  Command
      rd_RDA1  =  read[GATEWAY_RDA1],   // Read Data 1
      rd_RDA0  =  read[GATEWAY_RDA0],   // Read Data 0
      rd_RESP  =  read[GATEWAY_RESP],   // Response
      rd_TIMO  =  read[GATEWAY_TIMO],   // Time Out
      rd_TIMB  =  read[GATEWAY_TIMB],   // Timer_Base
      rd_TIMR  =  read[GATEWAY_TIMR],   // Timer
      rd_FLAG  =  read[GATEWAY_FLAG];   // Flag
// Swaps
wire  ld_BUSY  = ws & decode[GATEWAY_BUSY],   // Write or Swap BUSY  0: Free -1: BUSY
      rd_BUSY  = rs & decode[GATEWAY_BUSY];   // Read  or Swap BUSY

// State Machine ---------------------------------------------------------------------------------------------------------------

reg   [4:0]  EFSM;   localparam [2:0]   IDLE = 0,        WAIT = 1,        SEND = 2,        RSPN = 3,        FINI = 4;
                     localparam [4:0]  sIDLE = 5'b00001,sWAIT = 5'b00010,sSEND = 5'b00100,sRSPN = 5'b01000,sFINI = 5'b10000;

always @(posedge clock or posedge reset)
   if (reset)                          EFSM  <= `tCQ sIDLE;
   else case (1'b1)
      EFSM[IDLE] : if ( ld_CMND     )  EFSM  <= `tCQ sWAIT;    // ld_CMND kicks off the FSM
      EFSM[WAIT] : if (!pop         )  EFSM  <= `tCQ sSEND;    // if the coast is clear send a request
      EFSM[SEND] : if ( pop         )  EFSM  <= `tCQ sRSPN;    // Wait for the request to be taken
      EFSM[RSPN] : if ( time0       )  EFSM  <= `tCQ sIDLE;    // Wait for a response.   Timeout: give up!!
              else if ( push        )  EFSM  <= `tCQ sFINI;    // Wait for a response
      EFSM[FINI] :                     EFSM  <= `tCQ sIDLE;    // Send an interrupt to the processor and goto idle
      default                          EFSM  <= `tCQ sIDLE;
      endcase

wire     idle = EFSM[IDLE];
wire     send = EFSM[SEND];
wire     rspn = EFSM[RSPN];
wire     fini = EFSM[FINI];

// Registers -------------------------------------------------------------------------------------------------------------------

assign   time0 =  ~|TIMR[31:0];

always @(posedge clock)    if ( reset           )  TIMB  <= `tCQ {32{1'b1}};
                      else if (         ld_TIMB )  TIMB  <= `tCQ  iIO[31:0];

always @(posedge clock)    if ( reset           )  TIMR  <= `tCQ {32{1'b1}};
                      else if ( idle && ld_TIMR )  TIMR  <= `tCQ  iIO;
                      else if ( idle && ld_CMND )  TIMR  <= `tCQ  TIMB;
                      else if ( rspn && !time0  )  TIMR  <= `tCQ  TIMR -1;

always @(posedge clock)    if ( idle && ld_ADDR )  ADDR  <= `tCQ  iIO[31:0];
always @(posedge clock)    if ( idle && ld_DATA )  DATA  <= `tCQ  iIO[31:0];
always @(posedge clock)    if ( idle && ld_CMND )  CMND  <= `tCQ {iIO[31:8],myID[7:0]};

always @(posedge clock)    if ( reset           )  BUSY  <= `tCQ  1'b0;
                      else if ( idle && ld_BUSY )  BUSY  <= `tCQ  iIO[0];
                      else if ( idle && ld_CMND )  BUSY  <= `tCQ  1'b1;
                      else if ( fini            )  BUSY  <= `tCQ  1'b0;

always @(posedge clock)    if ( idle && ld_RDA1 )  RDA1  <= `tCQ  iIO[31:0];
                      else if ( fini            )  RDA1  <= `tCQ mosi[31:00];

always @(posedge clock)    if ( idle && ld_RDA0 )  RDA0  <= `tCQ  iIO[31:0];
                      else if ( fini            )  RDA0  <= `tCQ mosi[63:32];

always @(posedge clock)    if ( idle && ld_RESP )  RESP  <= `tCQ  iIO[31:0];
                      else if ( fini            )  RESP  <= `tCQ mosi[95:64];

always @(posedge clock)    if ( reset           )  TIMO  <= `tCQ 1'b0;
                      else if ( rspn &&   ~TIMO )  TIMO  <= `tCQ time0;       // Sets once and holds!
                      else if (         ld_TIMO )  TIMO  <= `tCQ iIO[0];      // until cleared here

always @(posedge clock)    if ( reset           )  FLAG  <= `tCQ 32'h0;
                      else if (         ld_FLAG )  FLAG  <= `tCQ iIO[0];

// Outputs ---------------------------------------------------------------------------------------------------------------------
wire  [31:0]   IOo;
A2_mux #(12,32) iIOO (.z(IOo[31:0]), .s(read),
                      .d({
                           FLAG,
                           TIMR,
                           TIMB,
                           {32{TIMO}},
                           RESP,
                           RDA0,
                           RDA1,
                           {32{BUSY}},
                           CMND,
                           DATA,
                           ADDR,
                           24'h0,myID
                           }));

wire     IOk         =  (rs | ws) & in_range;

assign   imiso       =  {IOk, IOo};

assign   intpt_fini  =  fini;
assign   intpt_timo  =  TIMO;
assign   flag        =  FLAG[0];
// Send
assign   miso[95:0]  = {CMND, DATA, ADDR};
assign   ordy        =  send;
// Receive Ready
assign   irdy        =  rspn;

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
