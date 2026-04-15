/*==============================================================================================================================

  Copyright © 2021..2024 Advanced Architectures

  All rights reserved
  Confidential Information
  Limited Distribution to Authorized Persons Only
  Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

  Project Name         : Scrypt ASIC

  Description          : NEWS
                         One section of the interconnect system at the ASIC level.
                         This module connects a North, East, West and South I/O interfaces

================================================================================================================================
  THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
  IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
  BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
  TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
  AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
  ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
===============================================================================================================================*/
//
// Timing:
//    LVDS 2.5Gb/s 8b10b => 250 MB/s => 1 32-bit word every 64 cycles to A2S at 4GHz
//
// Registers in A2S I/O space:
//    0     Nhead    North Input Current data (no POP!)      head of North input FIFO with destination info for routing
//    1     Ehead    East  Input Current data (no POP!)
//    2     Whead    West  Input Current data (no POP!)
//    3     Shead    South Input Current data (no POP!)
//    4     Ncsr     North Control Status \               7 6 5 4 3 2 1 0
//    5     Ecsr     East  Control Status  \             +-+-+-+-+-+-+-+-+
//    6     Wcsr     West  Control Status   >------->    |A|B|P|L|N|E|W|S| LNEWS: destination bits set by A2S cleared by EoF
//    7     Scsr     South Control Status  /             +-+-+-+-+-+-+-+-+
//    8     Lcsr     Local Control Status /                \ \ \ \_Local
//    9     Local 32 bit Input Data Write Port /            \ \ \__Plain Text - Disable for Plain Text Part of Message
//    10    Control in Read (Control Input Pop)              \ \___Broadcast
//    11    NControl North Control Output                     \____Active - set by A2S cleared by End-of-Frame (EoF)
//    12    EControl East Control Output
//    13    WControl West Control Output
//    14    SControl South Control Output
//    15    Local 32 bit Output Read Port
//    16    NOutput  North Data Output
//    17    EOutput  East Data Output
//    18    WOutput  West Data Output
//    19    SOutput  South Data Output
//    20    NEWS_L_DMA_INCOMING            //Add later from Roger
//    21    NEWS_L_DMA_OUTGOING            //Add later from Roger
//    22    NEWS_DMA_STATE
//
//    Bug Fix 2_11_25 -> Found logic error in LVDS read interface (wNI,WEI,wWI,wSI) during failing packet data reads interlaced with gpio reads
//    Bug Fix 2_13_25 -> Fix Boundary on Control Output vs Data Output
//    Feature Update -> Add Recrypt
//    Bug Fix 3_2_25 -> Fix assignment file positioning to fix ncverilog compiler errors
//    Bug Fix 3_6_25 -> Remove duplicate wire declaration to fix ncverilog compiler errors
//
//==============================================================================================================================

//JL - Use Romeo convention for Simulation vs Synthesis Include Configurations
`ifdef synthesis
	 `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_defines.vh" //Comment out until Romeo ready for synthesis of compilable Top Netlist
	 //`include "/proj/TekStart/lokotech/soc/users/jfechter/cognitum_src_a0/src/include/A2_defines.vh" //Use local repo copy until Romeo's directory is updated to repo tip
`else
	`include "A2_defines.vh" //JLPL - 11_5_25 -> Fix simulation define statement
`endif
//End JL

module NEWS #(
   parameter   [12:0]   BASE = 13'h00000,             // Base Address of Module
   parameter            BAUD = 1,
   parameter            FREQ = 1
   )(
   // NEWS FIFO Output Interfaces
   output wire [ 8:0]   NOo,  // Data Outputs
   output wire [ 8:0]   EOo,  // Data Outputs
   output wire [ 8:0]   WOo,  // Data Outputs
   output wire [ 8:0]   SOo,  // Data Outputs
   input wire          NOor, EOor, WOor, SOor, // Output Ready - Change to Input for Mike's LVDS "Input Ready" Signal Connection //12_18_24 - Remove LOor
   output wire         rNO,  rEO,  rWO,  rSO,  rLO,   // Write - Change to Output for Mike's LVDS "Write" Signal Connection
   // NEWS FIFO Input  Interfaces
   input  wire [ 8:0]  iNI,   // Data Input
   input  wire [ 8:0]  iEI,   // Data Input
   input  wire [ 8:0]  iWI,   // Data Input
   input  wire [ 8:0]  iSI,   // Data Input
   input  wire [ 7:0]  iLI,   // Data Inputs
   output  wire        wNI,  wEI,  wWI,  wSI,  wLI,   // Write - Change to Output for Mike's LVDS "Read" Signal Connection
   input wire          NIir, EIir, WIir, SIir, LIir, // input ready - Change to Input for Mike's LVDS "Output Ready" Signal Connection
   // A2S Interface
   output wire [32:0]   imiso,                        // I/O interface to A2S
   input  wire [47:0]   imosi,                        // I/O interface from A2S
   // AES Encryption Block Input Interface   
   input  wire [40:0]   amiso,      // AES output   41 {ECBor, ECBad[3:0], ECBpo[3:0], ECB[31:0]} NB ir belongs to nmosi //JLSW
   output wire          amiso_rd,   // Read / Take  (Selected destination is ready!)
   // AES Request ECB Interface
   output wire [ 2:0]   rmosi,      // AES requests
   output wire          rmosi_wr,
   input  wire          rmosi_ir,
   output wire          parity_error, //JLPL - 11_8_25
   // Memory Interface                 // Memory is a slave
   output wire [66:0]   lmosi,                                             // 67 = {lreq, lwr, lrd, lwrd[31:0], ladr[31:0]} //JLSW -> To RAM_3PX DMA Port in TileZero //JLPL - 8_27_25
   input  wire [33:0]   lmiso,                                             //      {lack, lgnt, lrdd[31:0]}  = 34 //JLSW       -> From RAM_3Pax DMA Port in TileZero //JLPL - 8_27_25
   // DMA Outgoing Complete INTERRUPT  //JLPL - 9_6_25
   output wire          intpt_ldma,    // JLPL - 9_6_25 - DMA Outgoing Transfer Complete interrupt   
   // Global
   output wire [10:0]   cc,                           // Condition codes
   output wire          intpt,                        // Interrupt
   input  wire          reset,
   input  wire          clock
   );



localparam  A  = 7, B = 6, F = 5, L = 4, N = 3, E = 2, W = 1, S = 0;    // Direction Control bits
localparam  [3:0]  SOF = 3, EOF = 2, BRK = 1, ERR = 0;                  // Status bits

//JL - Use Romeo convention for Simulation vs Synthesis Include Configurations
`ifdef synthesis
	 `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_IO_addresses.vh" //Comment out until Romeo ready for synthesis of compilable Top Netlist
	 //`include "/proj/TekStart/lokotech/soc/users/jfechter/cognitum_a0/src/include/COGNITUM_IO_addresses.vh" //Use local repo copy until Romeo's directory is updated to repo tip
`else
	`include "COGNITUM_IO_addresses.vh"
`endif
//End JL

`define LG2REGS 5  //JL Temporary
`define NUM_REGS                      23  //JL - Number of NEWS Registers //JLPL - 8_18_25 - 23 with DMA registers
//localparam  LG2REGS  =  `LG2(NUM_REGS); //JL //JL Temporary
`define CTRL_SOF 8'hFB   //Control Input Start of Frame
`define CTRL_EOF 8'hFD   //Control Input End of Frame

wire  [31:0]   NIo,  EIo,  WIo,  SIo; 

// Locals =======================================================================================================================

// Physical Flip-flops
reg   [31:0]   Ucsr;
reg   [17:0]   LOdma, LIdma;
reg   [16:0]   LOcnt, LIcnt;
//reg   [ 7:0]   Ncsr,  Ecsr,  Wcsr,  Scsr,  Lcsr; //JLPL - 8_11_25
reg   [ 7:0]   Lcsr; //JLPL - 8_11_25 - 8_12_25 - 8_13_25
reg   [ 8:0]   Ncsr, Scsr, Ecsr, Wcsr; //JLPL - 8_11_25 - 8_12_25 - 8_13_25

reg   [25:0]   Nlvc,  Elvc,  Wlvc,  Slvc;             // lvds compensation settings

wire           UObr = 1'b1; //JLPL - 8_28_25 - Set to default value for Roger simulation - This function not being used presently
//wire           clrDMAo, advDMAo, advDMAi, endDMAo; //JLPL - 8_28_25 Comment Out

// imosi\imiso Interface decoding ===============================================================================================

wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
// Decollate imosi
assign   {cIO, iIO, aIO} = imosi;

wire                  in_range =  aIO[12:`LG2REGS]  == BASE[12:`LG2REGS];             //Changed to defines
wire  [`NUM_REGS-1:0]     write =  ((cIO==3'b110) & in_range)  << aIO[`LG2REGS-1:0];  //Changed to defines
wire  [`NUM_REGS-1:0]      read =  ((cIO==3'b101) & in_range)  << aIO[`LG2REGS-1:0];  //Changed to defines


wire [31:0] dma_read_incoming; //JLPL - 8_18_25
wire [31:0] dma_read_outgoing; //JLPL - 8_18_25
wire [31:0] dma_read_state; //JLPL - 8_18_25
wire [31:0] interrupt_readback; //JLPL - 9_16_25
//JLPL - 9_16_25 -> Add Decryption Frame Detection to Enable Security Function
reg [2:0] south_decryption_enable_state;
reg [2:0] north_decryption_enable_state;
reg [2:0] east_decryption_enable_state;
reg [2:0] west_decryption_enable_state;
reg north_decryption_enable;
reg east_decryption_enable;
reg west_decryption_enable;
reg south_decryption_enable;
wire north_decrypt_active;
wire east_decrypt_active;
wire west_decrypt_active;
wire south_decrypt_active;
//End JLPL - 9_16_25
//JLPL - 9_17_25 -> Add Encryption Frame Detection to Enable Security Function
reg [2:0] south_encryption_enable_state;
reg [2:0] north_encryption_enable_state;
reg [2:0] east_encryption_enable_state;
reg [2:0] west_encryption_enable_state;
reg north_encryption_enable;
reg east_encryption_enable;
reg west_encryption_enable;
reg south_encryption_enable;
wire north_encrypt_active;
wire east_encrypt_active;
wire west_encrypt_active;
wire south_encrypt_active;
//End JLPL - 9_17_25
wire LNO,LOor; //JLPL - 7_31_25 - Declare Earlier for compiler

wire           NIor, EIor, WIor, SIor, LIor;

wire N_control_pipe_ready;
wire [7:0] N_control_out;
wire N_control_data_ready;
wire N_control_read;
wire E_control_pipe_ready;
wire [7:0] E_control_out;
wire E_control_data_ready;
wire E_control_read;
wire W_control_pipe_ready;
wire [7:0] W_control_out;
wire W_control_data_ready;
wire W_control_read;
wire S_control_pipe_ready;
wire [7:0] S_control_out;
wire S_control_data_ready;
wire S_control_read;

wire [8:0] north_output_pipe_in;
wire north_control_mux_active;
wire N_output_pipe_ready;
wire N_output_pipe_write;


wire [7:0] NOo_data_out;
wire rNO_data_out;
wire north_output_empty_flag;

wire [7:0] EOo_data_out;
wire rEO_data_out;
wire [8:0] east_output_pipe_in;
wire east_control_mux_active;
wire E_output_pipe_ready;
wire E_output_pipe_write;

wire [7:0] WOo_data_out;
wire rWO_data_out;
wire [8:0] west_output_pipe_in;
wire west_control_mux_active;
wire W_output_pipe_ready;
wire W_output_pipe_write;

wire [7:0] SOo_data_out;
wire rSO_data_out;
wire [8:0] south_output_pipe_in;
wire south_control_mux_active;
wire S_output_pipe_ready;
wire S_output_pipe_write;

reg N_EOF_CLEAR = 1'b0;
reg E_EOF_CLEAR = 1'b0;
reg W_EOF_CLEAR = 1'b0;
reg S_EOF_CLEAR = 1'b0;
wire N_EOF_Write;
wire E_EOF_Write;
wire W_EOF_Write;
wire S_EOF_Write;

assign east_control_mux_active = E_control_data_ready & !rEO_data_out;
assign E_control_read = east_control_mux_active & E_output_pipe_ready;
assign E_output_pipe_write = ( east_control_mux_active | rEO_data_out ) & E_output_pipe_ready;

assign west_control_mux_active = W_control_data_ready & !rWO_data_out;
assign W_control_read = west_control_mux_active & W_output_pipe_ready;
assign W_output_pipe_write = ( west_control_mux_active | rWO_data_out ) & W_output_pipe_ready;

assign south_control_mux_active = S_control_data_ready & !rSO_data_out;
assign S_control_read = south_control_mux_active & S_output_pipe_ready;
assign S_output_pipe_write = ( south_control_mux_active | rSO_data_out ) & S_output_pipe_ready;

assign north_output_empty_flag = !rNO_data_out & !N_control_data_ready;

assign north_control_mux_active = N_control_data_ready & !rNO_data_out;
assign N_control_read = north_control_mux_active & N_output_pipe_ready;
assign N_output_pipe_write = ( north_control_mux_active | rNO_data_out ) & N_output_pipe_ready;

wire [7:0] N_control_in = N_EOF_Write == 1 ? 8'hFD : iIO[7:0];
wire N_control_in_write = write[NEWS_N_CONTROL] | N_EOF_Write;
wire [7:0] E_control_in = E_EOF_Write == 1 ? 8'hFD : iIO[7:0];
wire E_control_in_write = write[NEWS_E_CONTROL] | E_EOF_Write;
wire [7:0] W_control_in = W_EOF_Write == 1 ? 8'hFD : iIO[7:0];
wire W_control_in_write = write[NEWS_W_CONTROL] | W_EOF_Write;
wire [7:0] S_control_in = S_EOF_Write == 1 ? 8'hFD : iIO[7:0];
wire S_control_in_write = write[NEWS_S_CONTROL] | S_EOF_Write;

A2_pipe     #(8) i_N_control (.pdi(N_control_in),.pir(N_control_pipe_ready),.pwr(N_control_in_write), .pdo(N_control_out),.por(N_control_data_ready),.prd(N_control_read),.clock(clock),.reset(reset)); 
A2_pipe     #(8) i_E_control (.pdi(E_control_in),.pir(E_control_pipe_ready),.pwr(E_control_in_write), .pdo(E_control_out),.por(E_control_data_ready),.prd(E_control_read),.clock(clock),.reset(reset)); 
A2_pipe     #(8) i_W_control (.pdi(W_control_in),.pir(W_control_pipe_ready),.pwr(W_control_in_write), .pdo(W_control_out),.por(W_control_data_ready),.prd(W_control_read),.clock(clock),.reset(reset));
A2_pipe     #(8) i_S_control (.pdi(S_control_in),.pir(S_control_pipe_ready),.pwr(S_control_in_write), .pdo(S_control_out),.por(S_control_data_ready),.prd(S_control_read),.clock(clock),.reset(reset));


assign north_output_pipe_in = north_control_mux_active == 1 ? {1'b1,N_control_out} : {1'b0,NOo_data_out};
assign east_output_pipe_in = east_control_mux_active == 1 ? {1'b1,E_control_out} : {1'b0,EOo_data_out};
assign west_output_pipe_in = west_control_mux_active == 1 ? {1'b1,W_control_out} : {1'b0,WOo_data_out};
assign south_output_pipe_in = south_control_mux_active == 1 ? {1'b1,S_control_out} : {1'b0,SOo_data_out};

A2_pipe     #(9) i_N_out (.pdi(north_output_pipe_in),.pir(N_output_pipe_ready),.pwr(N_output_pipe_write), .pdo(NOo),.por(rNO),.prd(NOor),.clock(clock),.reset(reset));
A2_pipe     #(9) i_E_out (.pdi(east_output_pipe_in),.pir(E_output_pipe_ready),.pwr(E_output_pipe_write), .pdo(EOo),.por(rEO),.prd(EOor),.clock(clock),.reset(reset));
A2_pipe     #(9) i_W_out (.pdi(west_output_pipe_in),.pir(W_output_pipe_ready),.pwr(W_output_pipe_write), .pdo(WOo),.por(rWO),.prd(WOor),.clock(clock),.reset(reset));
A2_pipe     #(9) i_S_out (.pdi(south_output_pipe_in),.pir(S_output_pipe_ready),.pwr(S_output_pipe_write), .pdo(SOo),.por(rSO),.prd(SOor),.clock(clock),.reset(reset));


always @(posedge clock) if (reset || N_EOF_CLEAR)   Ncsr  <= #1 9'h0; //Change End of Frame Register Clear to N_EOF_CLEAR //JLPL - 8_11_25 now 9 bits
   else if ( write[NEWS_N_CSR])              Ncsr  <= #1 iIO[8:0]; //JLPL - 8_11_25 now 9 bits

always @(posedge clock) if (reset || E_EOF_CLEAR)   Ecsr  <= #1 9'h0; //Change End of Frame Register Clear to E_EOF_CLEAR //JLPL - 8_13_25 now 9 bits
   else if ( write[NEWS_E_CSR])              Ecsr  <= #1 iIO[8:0]; //JLPL - 8_13_25 now 9 bits 

always @(posedge clock) if (reset || W_EOF_CLEAR)   Wcsr  <= #1 9'h0; //Change End of Frame Register Clear to W_EOF_CLEAR //JLPL - 8_13_25 now 9 bits 
   else if ( write[NEWS_W_CSR])              Wcsr  <= #1 iIO[8:0]; //JLPL - 8_13_25 now 9 bits 

always @(posedge clock) if (reset || S_EOF_CLEAR)   Scsr  <= #1 9'h0; //Change End of Frame Register Clear to S_EOF_CLEAR //JLPL - 8_12_25 now 9 bits
   else if ( write[NEWS_S_CSR])              Scsr  <= #1 iIO[8:0]; //JLPL - 8_12_25 now 9 bits 

always @(posedge clock) if (reset)           Lcsr  <= #1 8'h0;
   else if ( write[NEWS_L_CSR])              Lcsr  <= #1 iIO; 
   
//Add A2S Local 32 Bit Output Port Write Logic -> Write to Local Pipe and then this will be re-directed to LVDS Port based on LCSR directional port settings
wire local_input_pipe_ready;




//A2_mux iIOmx (.z(IOo[31:0]), .s(read), .d({NIo,EIo,WIo,SIo,24'h0,Ncsr,24'h0,Ecsr,24'h0,Wcsr,24'h0,Scsr,24'h0,Lcsr})); //Comment Out - Discrete Mux Below

//3_2_25 -> Compiler errors - wire must be defined before using wire in assigns so move before use
wire [7:0] north_input_control_pipe_out;
wire [7:0] east_input_control_pipe_out;
wire [7:0] west_input_control_pipe_out;
wire [7:0] south_input_control_pipe_out;
wire [31:0] LOo;
reg [31:0] north_output_write_readback;
reg [31:0] east_output_write_readback;
reg [31:0] west_output_write_readback;
reg [31:0] south_output_write_readback;
//End 3_2_25

wire [31:0] control_input_readback;
wire  [31:0]   LIo;
assign control_input_readback = {north_input_control_pipe_out,east_input_control_pipe_out,west_input_control_pipe_out,south_input_control_pipe_out};

assign IOo[31:0] = read[NEWS_N_HEAD] == 1 ? NIo :
                         read[NEWS_E_HEAD] == 1 ? EIo :
                         read[NEWS_W_HEAD] == 1 ? WIo :
                         read[NEWS_S_HEAD] == 1 ? SIo :
                         read[NEWS_N_CSR] == 1 ? Ncsr :
                         read[NEWS_E_CSR] == 1 ? Ecsr :
                         read[NEWS_W_CSR] == 1 ? Wcsr :
                         read[NEWS_S_CSR] == 1 ? Scsr :
                         read[NEWS_L_CSR] == 1 ? Lcsr :
                         read[NEWS_L_IN] == 1 ? LIo :
                         read[NEWS_L_EOF] == 1 ? control_input_readback :
                         read[NEWS_N_CONTROL] == 1 ? N_control_out :
                         read[NEWS_E_CONTROL] == 1 ? E_control_out :
                         read[NEWS_W_CONTROL] == 1 ? W_control_out :
                         read[NEWS_S_CONTROL] == 1 ? S_control_out :
                         read[NEWS_L_OUT]      == 1 ? LOo :  //A2S reads full packet from local output pipe
                         read[NEWS_N_OUTPUT]   == 1 ? north_output_write_readback :  
                         read[NEWS_E_OUTPUT]   == 1 ? east_output_write_readback :  
                         read[NEWS_W_OUTPUT]   == 1 ? west_output_write_readback :  
                         read[NEWS_S_OUTPUT]   == 1 ? south_output_write_readback :  
                         read[NEWS_L_DMA_INCOMING]   == 1 ? dma_read_incoming :   //JLPL - 8_18_25
                         read[NEWS_L_DMA_OUTGOING]   == 1 ? dma_read_outgoing :   //JLPL - 8_18_25
                         read[NEWS_L_DMA_STATE]   == 1 ? interrupt_readback :   //JLPL - 9_16_25						 
                         32'h0;


//assign   IOk   =  (rd | wr) & in_range; //Comment out
assign  IOk  =  |write[`NUM_REGS-1:0] |  |read[`NUM_REGS-1:0];
// Collate imiso
assign   imiso =  {IOk, IOo};

// Check for illegal connections.  Detect multiple inputs concurrently trying to access the same output =========================
wire  [4:0]    flag;
assign         flag[L] = (Ncsr[L] + Ecsr[L] + Wcsr[L] + Scsr[L] + Lcsr[L]) > 1, //Changed to L instead of C
               flag[N] = (Ncsr[N] + Ecsr[N] + Wcsr[N] + Scsr[N] + Lcsr[N]) > 1,
               flag[E] = (Ncsr[E] + Ecsr[E] + Wcsr[E] + Scsr[E] + Lcsr[E]) > 1,
               flag[W] = (Ncsr[W] + Ecsr[W] + Wcsr[W] + Scsr[W] + Lcsr[W]) > 1,
               flag[S] = (Ncsr[S] + Ecsr[S] + Wcsr[S] + Scsr[S] + Lcsr[S]) > 1;

// Start-of-frame & End-of-frame Outputs ========================================================================================


// Local and Input PIPE Buffers =================================================================================================

//wire  [31:0]   LMo; //JLPL - 8_28_25 Comment Out 
//wire         siLI, eiLI; //JLPL - 8_28_25 Comment Out

wire          rNI,  rEI,  rWI,  rSI,  rLI;

//Move Declaration of Variables -- Compiler Version
//wire  [31:0]  iLO,  iNO,  iEO,  iWO,  iSO; //Remove duplicate declaration LOo //JLPL - 8_28_25 Comment Out
wire           LOir, NOir, EOir, WOir, SOir;

//Fix Pipe Interface Signals to be compatible with LVDS Interface Design
wire north_input_pipe_ready,east_input_pipe_ready,west_input_pipe_ready,south_input_pipe_ready;
wire N_input_control_pipe_ready; 
wire E_input_control_pipe_ready; 
wire W_input_control_pipe_ready; 
wire S_input_control_pipe_ready; 

wire write_north_internal = NIir & north_input_pipe_ready & !iNI[8]; //Change Block to command signal
wire write_north_external = NIir & ( (north_input_pipe_ready & !iNI[8]) | (N_input_control_pipe_ready & iNI[8]) ); 
wire write_east_internal = EIir & east_input_pipe_ready & !iEI[8]; 
wire write_east_external = EIir & ( (east_input_pipe_ready & !iEI[8]) | (E_input_control_pipe_ready & iEI[8]) );
wire write_west_internal = WIir & west_input_pipe_ready & !iWI[8];
wire write_west_external = WIir & ( (west_input_pipe_ready & !iWI[8]) | (W_input_control_pipe_ready & iWI[8]) );
wire write_south_internal = SIir & south_input_pipe_ready & !iSI[8];
wire write_south_external = SIir & ( (south_input_pipe_ready & !iSI[8]) | (S_input_control_pipe_ready & iSI[8]) );
wire write_local_internal = LIir & local_input_pipe_ready;

assign wNI = write_north_external;
assign wEI = write_east_external;
assign wWI = write_west_external;
assign wSI = write_south_external;
assign wLI = write_local_internal;


//                        ------- Input to NEWS -------   ---- Output to Crossbar -----
A2_pipe_1toN #(8,4,1) i_NI (.pdi(iNI[7:0]),.pir(north_input_pipe_ready),.pwr(write_north_internal), .pdo(NIo),.por(NIor),.prd(rNI),.flush(1'b0), .clock(clock),.reset(reset)); //Endianess to Big Endian
A2_pipe_1toN #(8,4,1) i_EI (.pdi(iEI[7:0]),.pir(east_input_pipe_ready),.pwr(write_east_internal), .pdo(EIo),.por(EIor),.prd(rEI),.flush(1'b0), .clock(clock),.reset(reset)); //Endianess to Big Endian
A2_pipe_1toN #(8,4,1) i_WI (.pdi(iWI[7:0]),.pir(west_input_pipe_ready),.pwr(write_west_internal), .pdo(WIo),.por(WIor),.prd(rWI),.flush(1'b0), .clock(clock),.reset(reset)); //Endianess to Big Endian
A2_pipe_1toN #(8,4,1) i_SI (.pdi(iSI[7:0]),.pir(south_input_pipe_ready),.pwr(write_south_internal), .pdo(SIo),.por(SIor),.prd(rSI),.flush(1'b0), .clock(clock),.reset(reset)); //Endianess to Big Endian
//A2_pipe     #(8*4) i_LI (.pdi(iIO),.pir(local_input_pipe_ready),.pwr(write[NEWS_L_IN]), .pdo(LIo),.por(LIor),.prd(rLI),            .clock(clock),.reset(reset)); //Endianess to Big Endian //JLPL - 8_25_25

wire DMA_LOor; //JLPL - 8_25_25 Move earlier for compiler
wire [31:0] LOdo; //JLPL - 8_25_25 Move earlier for compiler

A2_pipe     #(8*4) i_LI (.pdi(LOdo),.pir(local_input_pipe_ready),.pwr(DMA_LOor), .pdo(LIo),.por(LIor),.prd(rLI),            .clock(clock),.reset(reset)); //Endianess to Big Endian //JLPL - 8_25_25

wire N_input_control_pipe_write;
wire north_input_control_pipe_out_data_ready;
wire north_input_control_pipe_out_read;

wire E_input_control_pipe_write;
wire east_input_control_pipe_out_data_ready;
wire east_input_control_pipe_out_read;

wire W_input_control_pipe_write;
wire west_input_control_pipe_out_data_ready;
wire west_input_control_pipe_out_read;

wire S_input_control_pipe_write;
wire south_input_control_pipe_out_data_ready;
wire south_input_control_pipe_out_read;

wire N_Ctrl_SOF = north_input_control_pipe_out_data_ready & ( north_input_control_pipe_out == `CTRL_SOF );
wire E_Ctrl_SOF = east_input_control_pipe_out_data_ready & ( east_input_control_pipe_out == `CTRL_SOF );
wire W_Ctrl_SOF = west_input_control_pipe_out_data_ready & ( west_input_control_pipe_out == `CTRL_SOF );
wire S_Ctrl_SOF = south_input_control_pipe_out_data_ready & ( south_input_control_pipe_out == `CTRL_SOF );
wire N_Ctrl_EOF = north_input_control_pipe_out_data_ready & ( north_input_control_pipe_out == `CTRL_EOF );
wire E_Ctrl_EOF = east_input_control_pipe_out_data_ready & ( east_input_control_pipe_out == `CTRL_EOF );
wire W_Ctrl_EOF = west_input_control_pipe_out_data_ready & ( west_input_control_pipe_out == `CTRL_EOF );
wire S_Ctrl_EOF = south_input_control_pipe_out_data_ready & ( south_input_control_pipe_out == `CTRL_EOF );

wire N_EOF = (N_Ctrl_EOF & Ncsr[A] & Ncsr[N]) | (E_Ctrl_EOF & Ecsr[A] & Ecsr[N]) | (W_Ctrl_EOF & Wcsr[A] & Wcsr[N]) | (S_Ctrl_EOF & Scsr[A] & Scsr[N]);
wire E_EOF = (N_Ctrl_EOF & Ncsr[A] & Ncsr[E]) | (E_Ctrl_EOF & Ecsr[A] & Ecsr[E]) | (W_Ctrl_EOF & Wcsr[A] & Wcsr[E]) | (S_Ctrl_EOF & Scsr[A] & Scsr[E]);
wire W_EOF = (N_Ctrl_EOF & Ncsr[A] & Ncsr[W]) | (E_Ctrl_EOF & Ecsr[A] & Ecsr[W]) | (W_Ctrl_EOF & Wcsr[A] & Wcsr[W]) | (S_Ctrl_EOF & Scsr[A] & Scsr[W]);
wire S_EOF = (N_Ctrl_EOF & Ncsr[A] & Ncsr[S]) | (E_Ctrl_EOF & Ecsr[A] & Ecsr[S]) | (W_Ctrl_EOF & Wcsr[A] & Wcsr[S]) | (S_Ctrl_EOF & Scsr[A] & Scsr[S]);

reg N_EOF_State1 = 1'b0;
reg E_EOF_State1 = 1'b0;
reg W_EOF_State1 = 1'b0;
reg S_EOF_State1 = 1'b0;
reg N_EOF_State2 = 1'b0;
reg E_EOF_State2 = 1'b0;
reg W_EOF_State2 = 1'b0;
reg S_EOF_State2 = 1'b0;

assign N_EOF_Write = N_EOF_State1 & !N_EOF_State2;
assign E_EOF_Write = E_EOF_State1 & !E_EOF_State2;
assign W_EOF_Write = W_EOF_State1 & !W_EOF_State2;
assign S_EOF_Write = S_EOF_State1 & !S_EOF_State2;

always @(posedge clock, posedge reset)
    begin
        if ( reset )
            begin
                N_EOF_State1 <= 1'b0;
                E_EOF_State1 <= 1'b0;
                W_EOF_State1 <= 1'b0;
                S_EOF_State1 <= 1'b0;
                N_EOF_State2 <= 1'b0;
                E_EOF_State2 <= 1'b0;
                W_EOF_State2 <= 1'b0;
                S_EOF_State2 <= 1'b0;
                N_EOF_CLEAR <= 1'b0;
                E_EOF_CLEAR <= 1'b0;
                W_EOF_CLEAR <= 1'b0;
                S_EOF_CLEAR <= 1'b0;
            end //end if reset
        else
            begin
                N_EOF_State2 <= N_EOF_State1;
                N_EOF_State1 <= N_EOF & !rNO_data_out;
                E_EOF_State2 <= E_EOF_State1;
                E_EOF_State1 <= E_EOF & !rEO_data_out;
                W_EOF_State2 <= W_EOF_State1;
                W_EOF_State1 <= W_EOF & !rWO_data_out;
                S_EOF_State2 <= S_EOF_State1;
                S_EOF_State1 <= S_EOF & !rSO_data_out;
                N_EOF_CLEAR <= N_EOF_Write;
                E_EOF_CLEAR <= E_EOF_Write;
                W_EOF_CLEAR <= W_EOF_Write;
                S_EOF_CLEAR <= S_EOF_Write;
            end //end else reset
    end //end always


assign N_input_control_pipe_write = NIir & iNI[8] & N_input_control_pipe_ready;
assign north_input_control_pipe_out_read = north_input_control_pipe_out_data_ready & ( read[NEWS_L_EOF] | N_EOF_CLEAR ); //Add Read 10 advancing of control port - Add N_EOF_CLEAR advancing of the control port ***
assign E_input_control_pipe_write = EIir & iEI[8] & E_input_control_pipe_ready;
assign east_input_control_pipe_out_read = east_input_control_pipe_out_data_ready & ( read[NEWS_L_EOF] | E_EOF_CLEAR ); //Read 10 or E_EOF_CLEAR advancing of the control port ***
assign W_input_control_pipe_write = WIir & iWI[8] & W_input_control_pipe_ready;
assign west_input_control_pipe_out_read = west_input_control_pipe_out_data_ready & ( read[NEWS_L_EOF] | W_EOF_CLEAR ); //Read 10 or W_EOF_CLEAR advancing of the control port ***
assign S_input_control_pipe_write = SIir & iSI[8] & S_input_control_pipe_ready;
assign south_input_control_pipe_out_read = south_input_control_pipe_out_data_ready & ( read[NEWS_L_EOF] | S_EOF_CLEAR ); //Read 10 or S_EOF_CLEAR advancing of the control port ***

A2_pipe     #(8) i_N_ctrl_in (.pdi(iNI[7:0]),.pir(N_input_control_pipe_ready),.pwr(N_input_control_pipe_write), .pdo(north_input_control_pipe_out),.por(north_input_control_pipe_out_data_ready),.prd(north_input_control_pipe_out_read),.clock(clock),.reset(reset));
A2_pipe     #(8) i_E_ctrl_in (.pdi(iEI[7:0]),.pir(E_input_control_pipe_ready),.pwr(E_input_control_pipe_write), .pdo(east_input_control_pipe_out),.por(east_input_control_pipe_out_data_ready),.prd(east_input_control_pipe_out_read),.clock(clock),.reset(reset));
A2_pipe     #(8) i_W_ctrl_in (.pdi(iWI[7:0]),.pir(W_input_control_pipe_ready),.pwr(W_input_control_pipe_write), .pdo(west_input_control_pipe_out),.por(west_input_control_pipe_out_data_ready),.prd(west_input_control_pipe_out_read),.clock(clock),.reset(reset));
A2_pipe     #(8) i_S_ctrl_in (.pdi(iSI[7:0]),.pir(S_input_control_pipe_ready),.pwr(S_input_control_pipe_write), .pdo(south_input_control_pipe_out),.por(south_input_control_pipe_out_data_ready),.prd(south_input_control_pipe_out_read),.clock(clock),.reset(reset));

wire FN_rd,FE_rd,FW_rd,FS_rd;
wire TN_rd,TE_rd,TW_rd,TS_rd;
wire FN_or,FE_or,FW_or,FS_or;
wire TN_or,TE_or,TW_or,TS_or;
wire [31:0] FNDBo,FEDBo,FWDBo,FSDBo;
wire [31:0] TNEBo,TEEBo,TWEBo,TSEBo;


wire wNO_muxed; //JLPL - 8_11_25 - Move here for compile
wire wSO_muxed;  //JLPL - 8_12_25 - Move here for compile
wire wEO_muxed; //JLPL - 8_13_25 - Move here for compile
wire wWO_muxed; //JLPL - 8_13_25 - Move here for compile

//JLPL - 9_16_25
`ifdef SIMULATE_CLOCK_RESET
assign north_decrypt_active = Ncsr[F];
assign east_decrypt_active = Ecsr[F];
assign west_decrypt_active = Wcsr[F];
assign south_decrypt_active = Scsr[F];
`else
assign north_decrypt_active = Ncsr[F] & north_decryption_enable;
assign east_decrypt_active = Ecsr[F] & east_decryption_enable;
assign west_decrypt_active = Wcsr[F] & west_decryption_enable;
assign south_decrypt_active = Scsr[F] & south_decryption_enable;
`endif
//End JLPL - 9_16_25
//JLPL - 9_17_25
`ifdef SIMULATE_CLOCK_RESET
assign north_encrypt_active = Ncsr[8];
assign east_encrypt_active = Ecsr[8];
assign west_encrypt_active = Wcsr[8];
assign south_encrypt_active = Scsr[8];
`else
assign north_encrypt_active = Ncsr[8] & north_encryption_enable;
assign east_encrypt_active = Ecsr[8] & east_encryption_enable;
assign west_encrypt_active = Wcsr[8] & west_encryption_enable;
assign south_encrypt_active = Scsr[8] & south_encryption_enable;
`endif
//End JLPL - 9_17_25

assign FN_rd = rNI & north_decrypt_active; //JLPL - 8_14_25 - Issue Read when Encryption is selected and North Input is passed to output //JLPL - 9_16_25
assign FE_rd = rEI & east_decrypt_active; //JLPL - 8_14_25 - Issue Read when Encryption is selected and East Input is passed to output //JLPL - 9_16_25
assign FW_rd = rWI & west_decrypt_active; //JLPL - 8_14_25 - Issue Read when Encryption is selected and West Input is passed to output //JLPL - 9_16_25
assign FS_rd = rSI & south_decrypt_active; //JLPL - 8_14_25 - Issue Read when Encryption is selected and South Input is passed to output //JLPL - 9_16_25
assign TN_rd = wNO_muxed & north_encrypt_active; //JLPL - 8_5_25 -> Add the Encryption after writing to the North Output //JLPL - 9_17_25
assign TE_rd = wEO_muxed & east_encrypt_active; //JLPL - 8_13_25 -> Add the Encryption after writing to the East Output //JLPL - 9_17_25
assign TW_rd = wWO_muxed & west_encrypt_active; //JLPL - 8_13_25 -> Add the Encryption after writing to the West Output //JLPL - 9_17_25
assign TS_rd = wSO_muxed & south_encrypt_active; //JLPL - 8_12_25 -> Add the Encryption after writing to the South Output //JLPL - 9_17_25


   Recrypt Recrypt_inst
     (.clock         (clock),
      .reset         (reset),
      .amiso         (amiso),
      .amiso_rd      (amiso_rd),   // Read / Take  (Selected destination is ready!)
      // AES Request ECB Interface
      .rmosi         (rmosi),      // AES requests
      .rmosi_wr      (rmosi_wr),
      .rmosi_ir      (rmosi_ir),
	  .parity_error  (parity_error), //JLPL - 11_8_25
      // NEWS Pipes
      .FNDBo         (FNDBo),      // From North Decrypt Block out to NEWS
      .FN_or         (FN_or),
      .FN_rd         (FN_rd),
      .FEDBo         (FEDBo),      // From East  Decrypt Block out to NEWS
      .FE_or         (FE_or),
      .FE_rd         (FE_rd),
      .FWDBo         (FWDBo),      // From West  Decrypt Block out to NEWS
      .FW_or         (FW_or),
      .FW_rd         (FW_rd),
      .FSDBo         (FSDBo),      // From South Decrypt Block out to NEWS
      .FS_or         (FS_or),
      .FS_rd         (FS_rd),
      .TNEBo         (TNEBo),      // To   North Encrypt Block out to NEWS
      .TN_or         (TN_or),
      .TN_rd         (TN_rd),
      .TEEBo         (TEEBo),      // To   East  Encrypt Block out to NEWS
      .TE_or         (TE_or),
      .TE_rd         (TE_rd),
      .TWEBo         (TWEBo),      // To   West  Encrypt Block out to NEWS
      .TW_or         (TW_or),
      .TW_rd         (TW_rd),
      .TSEBo           (TSEBo),      // To   South Encrypt Block out to NEWS
      .TS_or           (TS_or),
      .TS_rd           (TS_rd));     

//JLPL - 8_18_25 Add Memory DMA Interface

//wire DMA_LOor; //JLPL - 8_25_25 Move earlier for compiler
//wire [31:0] LOdo; //JLPL - 8_25_25 Move earlier for compiler
wire LOrd;
wire DMA_LIir;
//wire [31:0] LIdi = 32'h0;
//wire LIwr = 1'b0;
//wire [66:0] lmosi; //JLPL - 8_27_25
reg [33:0] lmiso_reg;
//wire [33:0] lmiso; //JLPL - 8_27_25
wire [31:0] RAMo;
wire [1:0] reg_write;

//8_20_25 - Test Only DMA Memory Output Transfers
wire [31:0] dma_out_address;
wire [31:0] dma_out_data;
wire dma_out_grnt;
wire dma_out_ack;
wire dma_out_req;
wire dma_out_wr;
assign dma_out_address = lmosi[31:0];
assign dma_out_data = lmosi[63:32];
assign dma_out_ack = lmiso[32];
assign dma_out_grnt = lmiso[33];
assign dma_out_req = lmosi[66];
assign dma_out_wr = lmosi[65];
//assign lmiso = {lmiso_reg[33],lmiso_reg[32],RAMo}; //JLPL - 8_22_25
//End 8_20_25
assign LOrd = DMA_LOor & local_input_pipe_ready; //JLPL - 8_25_25

assign reg_write[0] = write[NEWS_L_DMA_INCOMING];
assign reg_write[1] = write[NEWS_L_DMA_OUTGOING];

//8_20_25 - Test Only DMA Memory Output Transfers
always @(posedge clock)
	begin
		if ( reset )
			lmiso_reg <= 34'h200000000;
		else
			begin
				lmiso_reg[33] <= 1'b1;
				lmiso_reg[32] <= lmosi[66];
				if ( lmosi[65] )
					lmiso_reg[31:0] <= lmosi[63:32];
				else
					lmiso_reg[31:0] <= 32'h0;
			end //end else
	end //end always
//End 8_20_25

   NEWS_DMA NEWS_DMA_inst
   // Pipe Output Interface
     (.LOor         (DMA_LOor),
      .LOdo         (LOdo),
      .LOrd         (LOrd),
   // Pipe Input Interface
      .LIir         (DMA_LIir),
      .LIdi         (LOo), //JLPL - 8_20_25
      .LIwr         (LOor), //JLPL - 8_20_25
   // Memory Interface
      .lmosi        (lmosi),           // 67 {breq, bwr,brd,  bwrd[31:0], badr[31:0]}
      .lmiso        (lmiso),           // 34 {bgnt, back, brdd[31:0]}
   // Interrupt                        //JLPL - 9_6_25
      .intpt_ldma   (intpt_ldma),      //JLPL - 9_6_25
   // Register Interface
      .read_incoming (dma_read_incoming),
	  .read_outgoing (dma_read_outgoing),
	  .read_state    (dma_read_state),
      .ndmai         (iIO),
      .reg_write     (reg_write),
   // Global
      .clock           (clock),
      .reset           (reset));

//End JLPL - 8_18_25

//JLPL - 8_22_25 - Test TileZero DMA RAM
//JLPL - 8_27_25 - Comment Out at this level
//wire [65:0] hmiso; //JLPL - 8_27_25
//wire [33:0] bmiso;
//wire [66:0] hmosi = 67'h0;
//wire [66:0] bmosi = 67'h0;

//RAM_3Pax i_RAM3 (
//   .amiso(hmiso),
//   .bmiso(bmiso),
//   .cmiso(lmiso),
//   .amosi(hmosi),
//   .bmosi(bmosi),
//   .cmosi(lmosi),
//   .clock(clock)
//   );
//End JLPL - 8_22_25


//JLPL - 8_20_25 - Test DMA RAM

//wire [31:0] RAMo;
//JLPL - 8_27_25 - Comment Out
//RAM #(
//       .DEPTH  (2048),
//	   .WIDTH  (32)
//	 ) i_RAM (
//   .RAMo  (RAMo),
//   .iRAM  (lmosi[63:32]),
//   .aRAM  (lmosi[10:0]),
//   .eRAMn (~lmosi[66]),
//   .wRAMn (~lmosi[65]),
//   .clock (clock)
//   );
//End JLPL - 8_20_25


// Crossbar ---------------------------------------------------------------------------------------------------------------------
// 5x5 crossbar: Runs automagically.  Set-up and tear down, though, are done by A2S.

wire north_decrypt_ready = !north_decrypt_active | FN_or; //JLPL - 9_17_25
wire east_decrypt_ready = !east_decrypt_active | FE_or; //JLPL - 9_17_25
wire west_decrypt_ready = !west_decrypt_active | FW_or; //JLPL - 9_17_25
wire south_decrypt_ready = !south_decrypt_active | FS_or; //JLPL - 9_17_25
//wire north_decrypt_ready = 1'b1; //JLPL - 9_17_25
//wire east_decrypt_ready = 1'b1; //JLPL - 9_17_25
//wire west_decrypt_ready = 1'b1; //JLPL - 9_17_25
//wire south_decrypt_ready = 1'b1; //JLPL - 9_17_25
 

// for each Incoming 'output ready', if enabled, select destinations 'input ready' and create a request
wire     Nreq = (NIor & north_decrypt_ready) & Ncsr[A] & (Ncsr[N] & NOir | Ncsr[E] & EOir | Ncsr[W] & WOir | Ncsr[S] & SOir | Ncsr[L] & LOir), //Replace C with L //JLPL - 9_17_25
         Ereq = (EIor & east_decrypt_ready) & Ecsr[A] & (Ecsr[N] & NOir | Ecsr[E] & EOir | Ecsr[W] & WOir | Ecsr[S] & SOir | Ecsr[L] & LOir), //Replace C with L //JLPL - 9_17_25
         Wreq = (WIor & west_decrypt_ready) & Wcsr[A] & (Wcsr[N] & NOir | Wcsr[E] & EOir | Wcsr[W] & WOir | Wcsr[S] & SOir | Wcsr[L] & LOir), //Replace C with L //JLPL - 9_17_25
         Sreq = (SIor & south_decrypt_ready) & Scsr[A] & (Scsr[N] & NOir | Scsr[E] & EOir | Scsr[W] & WOir | Scsr[S] & SOir | Scsr[L] & LOir), //Replace C with L //JLPL - 9_17_25
         Lreq = LIor & Lcsr[A] & (Lcsr[N] & NOir | Lcsr[E] & EOir | Lcsr[W] & WOir | Lcsr[S] & SOir | Lcsr[L] & LOir); //Replace C with L //JLPL - 9_17_25

// Arbitrate 'square sparrow' amongst requests
wire  [7:0]    win;
A2_arbiter #(8) i_arb5 (.win(win),.req({3'b000,Lreq,Nreq,Ereq,Wreq,Sreq}),.adv(1'b1),.en(1'b1),.clock(clock),.reset(reset)); //Fix clock/reset signal sources - add proper advance/enable to this arbiter

// 'take' data from the input port that wins
assign   rLI  = win[L] | read[NEWS_L_IN], //JLPL - 8_26_25 - Add Read Local In I/O Advance
         rNI  = win[N],         
         rEI  = win[E],
         rWI  = win[W],
         rSI  = win[S]; 
         
//wire LNO,LOor; //JLPL - 7_31_25 - Declare Earlier for compiler
//assign LNO = read[NEWS_L_OUT] & LOor; //JLPL - 8_20_25
assign LNO = ( read[NEWS_L_OUT] | DMA_LIir ) & LOor; //JLPL - 8_20_25

// Winner then 'gives' to it's destination
wire     wLO  = win[L] & Lcsr[L] | win[N] & Ncsr[L] | win[E] & Ecsr[L] | win[W] & Wcsr[L] | win[S] & Scsr[L], //Replace C with L
         wNO  = win[L] & Lcsr[N] | win[N] & Ncsr[N] | win[E] & Ecsr[N] | win[W] & Wcsr[N] | win[S] & Scsr[N], //Replace C with L
         wEO  = win[L] & Lcsr[E] | win[N] & Ncsr[E] | win[E] & Ecsr[E] | win[W] & Wcsr[E] | win[S] & Scsr[E], //Replace C with L
         wWO  = win[L] & Lcsr[W] | win[N] & Ncsr[W] | win[E] & Ecsr[W] | win[W] & Wcsr[W] | win[S] & Scsr[W], //Replace C with L
         wSO  = win[L] & Lcsr[S] | win[N] & Ncsr[S] | win[E] & Ecsr[S] | win[W] & Wcsr[S] | win[S] & Scsr[S]; //Replace C with L

// Multiplex Data Inputs to Data Outputs ----------------------------------------------------------------------------------------
wire  [31:0]  NEWSo ;

wire  [31:0]  SIo_Decrypt; //JLPL - 7_31_25
wire  [31:0]  NIo_Decrypt; //JLPL - 8_3_25
wire  [31:0]  WIo_Decrypt; //JLPL - 8_4_25
wire  [31:0]  EIo_Decrypt; //JLPL - 8_5_25

assign SIo_Decrypt = SIo[31:0] ^ ( FSDBo & {32{south_decrypt_active}} ); //JLPL - 8_14_25 - Update to final From South Decrypt Input //JLPL - 9_16_25
assign NIo_Decrypt = NIo[31:0] ^ ( FNDBo & {32{north_decrypt_active}} ); //JLPL - 8_14_25 - Update to final From North Decrypt Input //JLPL - 9_16_25
assign WIo_Decrypt = WIo[31:0] ^ ( FWDBo & {32{west_decrypt_active}} ); //JLPL - 8_14_25 - Update to final From West Decrypt Input //JLPL - 9_16_25
assign EIo_Decrypt = EIo[31:0] ^ ( FEDBo & {32{east_decrypt_active}} ); //JLPL - 8_14_25 - Update to final From East Decrypt Input //JLPL - 9_16_25


A2_mux #(5,32) iNMX  //Fix Case Sensitive Module Callout - Add Proper Parameter Instantiation 5,32
    (.z(NEWSo[31:0]),.s(win[L:S]),.d({LIo[31:0],NIo_Decrypt[31:0],EIo_Decrypt[31:0],WIo_Decrypt[31:0],SIo_Decrypt[31:0]})); //Fixed Syntax //JLPL - 7_31_25 - Changed to Decrypt was SIo[31:0] //JLPL - 8_3_25 //JLPL - 8_4_25 //JLPL - 8_5_25

// Output PIPE Buffers ----------------------------------------------------------------------------------------------------------

//wire wNO_muxed; //JLPL - 8_11_25 - Move above for compile
//wire wEO_muxed; //JLPL - 8_13_25 - Move here for compile
//wire wWO_muxed; //JLPL - 8_13_25 - Move here for compile 
//wire wSO_muxed;  //JLPL - 8_12_25 - Move here for compile 
wire [31:0] NEWSo_N_muxed;
wire [31:0] NEWSo_E_muxed;
wire [31:0] NEWSo_W_muxed; 
wire [31:0] NEWSo_S_muxed; 

assign wNO_muxed = write[NEWS_N_OUTPUT] == 1 ? write[NEWS_N_OUTPUT] : wNO;
assign wEO_muxed = write[NEWS_E_OUTPUT] == 1 ? write[NEWS_E_OUTPUT] : wEO;
assign wWO_muxed = write[NEWS_W_OUTPUT] == 1 ? write[NEWS_W_OUTPUT] : wWO;
assign wSO_muxed = write[NEWS_S_OUTPUT] == 1 ? write[NEWS_S_OUTPUT] : wSO;
assign NEWSo_N_muxed = write[NEWS_N_OUTPUT] == 1 ? iIO : NEWSo;
assign NEWSo_E_muxed = write[NEWS_E_OUTPUT] == 1 ? iIO : NEWSo;
assign NEWSo_W_muxed = write[NEWS_W_OUTPUT] == 1 ? iIO : NEWSo;
assign NEWSo_S_muxed = write[NEWS_S_OUTPUT] == 1 ? iIO : NEWSo;

always @(posedge clock)
    begin
        if ( reset )
            begin
                north_output_write_readback <= 32'h0;
                east_output_write_readback <= 32'h0;
                west_output_write_readback <= 32'h0;
                south_output_write_readback <= 32'h0;              
            end //end of if reset
        else
            begin
               if ( write[NEWS_N_OUTPUT] )
                begin
                    if ( NOir )
                        north_output_write_readback <= iIO;
                    else
                        north_output_write_readback <= {iIO[15:0],iIO[31:16]};
                end //end of if write[NEWS_N_OUTPUT]
               if ( write[NEWS_E_OUTPUT] )
                begin
                    if ( EOir )
                        east_output_write_readback <= iIO;
                    else
                        east_output_write_readback <= {iIO[15:0],iIO[31:16]};
                end //end of if write[NEWS_E_OUTPUT] 
               if ( write[NEWS_W_OUTPUT] ) //**
                begin
                    if ( WOir )
                        west_output_write_readback <= iIO;
                    else
                        west_output_write_readback <= {iIO[15:0],iIO[31:16]};
                end //end of if write[NEWS_W_OUTPUT] 
               if ( write[NEWS_S_OUTPUT] ) //**
                begin
                    if ( SOir )
                        south_output_write_readback <= iIO;
                    else
                        south_output_write_readback <= {iIO[15:0],iIO[31:16]};
                end //end of if write[NEWS_S_OUTPUT]                                                              
            end //end else reset
    end //end always


wire [31:0] NEWSo_N_muxed_Encrypt; //JLPL - 8_5_25
wire [31:0] NEWSo_S_muxed_Encrypt; //JLPL - 8_12_25
wire [31:0] NEWSo_E_muxed_Encrypt; //JLPL - 8_13_25
wire [31:0] NEWSo_W_muxed_Encrypt; //JLPL - 8_13_25

assign NEWSo_N_muxed_Encrypt = NEWSo_N_muxed[31:0] ^ ( TNEBo & {32{north_encrypt_active}} ); //JLPL - 8_5_25 - Change Later from : "To" to "From" //JLPL - 9_17_25
assign NEWSo_S_muxed_Encrypt = NEWSo_S_muxed[31:0] ^ ( TSEBo & {32{south_encrypt_active}} ); //JLPL - 8_12_25 - Change Later from : "To" to "From" //JLPL - 9_17_25
assign NEWSo_E_muxed_Encrypt = NEWSo_E_muxed[31:0] ^ ( TEEBo & {32{east_encrypt_active}} ); //JLPL - 8_13_25 - Change Later from : "To" to "From" //JLPL - 9_17_25
assign NEWSo_W_muxed_Encrypt = NEWSo_W_muxed[31:0] ^ ( TWEBo & {32{west_encrypt_active}} ); //JLPL - 8_13_25 - Change Later from : "To" to "From" //JLPL - 9_17_25

//                        ----- Input from Crossbar -----   ------ Output to NEWS -------
A2_pipe_Nto1 #(8,4) i_NO (.pdi(NEWSo_N_muxed_Encrypt),.pir(NOir),.pwr(wNO_muxed), .pdo(NOo_data_out),.por(rNO_data_out),.prd(N_output_pipe_ready), .clock(clock),.reset(reset)); //JLPL - 8_5_25 was NEWSo_N_muxed
A2_pipe_Nto1 #(8,4) i_EO (.pdi(NEWSo_E_muxed_Encrypt),.pir(EOir),.pwr(wEO_muxed), .pdo(EOo_data_out),.por(rEO_data_out),.prd(E_output_pipe_ready), .clock(clock),.reset(reset)); //JLPL - 8_13_25 was NEWSo_E_muxed
A2_pipe_Nto1 #(8,4) i_WO (.pdi(NEWSo_W_muxed_Encrypt),.pir(WOir),.pwr(wWO_muxed), .pdo(WOo_data_out),.por(rWO_data_out),.prd(W_output_pipe_ready), .clock(clock),.reset(reset)); //JLPL - 8_13_25 was NEWSo_W_muxed
A2_pipe_Nto1 #(8,4) i_SO (.pdi(NEWSo_S_muxed_Encrypt),.pir(SOir),.pwr(wSO_muxed), .pdo(SOo_data_out),.por(rSO_data_out),.prd(S_output_pipe_ready), .clock(clock),.reset(reset)); //JLPL - 8_12_25 was NEWSo_S_muxed


A2_pipe #(8*4) i_LO (.pdi(NEWSo),.pir(LOir),.pwr(wLO), .pdo(LOo),.por(LOor),.prd(LNO), .clock(clock),.reset(reset)); //Use Base Local Out Pipe

// Break forwarding =============================================================================================================
// Select a single break on who comes first and latch on to it
//    Distribute break out to all other ports
//    Ports opposite get the break immediately -------------------***** TBD *****
//    Ports on the diagonal get delayed --------------------------***** TBD *****
// When selected ports break releases
//    Release distributed breaks
//    Wait for all breaks to finish
//    Go to idle

wire  [3:0] NEWSbreak   =  {1'b0, 1'b0, 1'b0, 1'b0};
wire  [3:0] NEWSextra   =  {1'b0, 1'b0, 1'b0, 1'b0};
wire        anybreak    =  |NEWSbreak[3:0];
wire         nobreak    =  ~anybreak;

reg   [3:0] NEWSroute;
always @* casez (NEWSbreak[3:0])
   4'b1???  :  NEWSroute = 1000;
   4'b01??  :  NEWSroute = 0100;
   4'b001?  :  NEWSroute = 0010;
   4'b0001  :  NEWSroute = 0001;
   default     NEWSroute = 0000;
   endcase

wire  breakoff =  |(~NEWSbreak & UObr);

reg   [1:0] BFSM; localparam[1:0] OFF = 2'd0, SETUP = 2'd1, CLOSE = 2'd2;

`define  CLOSE default
always @(posedge clock)
   if (reset)  BFSM  <= #1 OFF;
   else case  (BFSM)
      OFF   : if (anybreak)   BFSM <= #1 SETUP;
      SETUP : if (breakoff)   BFSM <= #1 CLOSE;
     `CLOSE : if ( nobreak)   BFSM <= #1 OFF;
      endcase

reg   [3:0] Break, Extra;
always @(posedge clock)
   if (reset || ((BFSM==SETUP) && breakoff)) Break <= #1 4'b0000;
   else if (     (BFSM==OFF  ) && anybreak ) Break <= #1 NEWSroute;


// Outputs ---------------------------------------------------------------------------------------------------------------------

assign intpt = north_input_control_pipe_out_data_ready | east_input_control_pipe_out_data_ready | west_input_control_pipe_out_data_ready | south_input_control_pipe_out_data_ready;

assign   cc[10:0] =  {north_output_empty_flag,flag[4:0],NIor,EIor,WIor,SIor,|flag[4:0]}; //Remove false comma at the end

assign interrupt_readback = {28'h0,north_input_control_pipe_out_data_ready,east_input_control_pipe_out_data_ready,west_input_control_pipe_out_data_ready,south_input_control_pipe_out_data_ready}; //JLPL - 9_16_25

//JLPL - 9_16_25 -> Add Decryption Frame Detection to Enable Security Function

//North Input Decryption Enable Detection
always @(posedge clock)
    begin
        if ( reset || !Ncsr[F] )
            begin
				north_decryption_enable_state <= 3'b000;
				north_decryption_enable <= 1'b0;
            end //end of if reset
		else
			begin
				if ( N_Ctrl_EOF )
					begin
						north_decryption_enable_state <= 3'b000;
						north_decryption_enable <= 1'b0;
					end //end of if N_Ctrl_EOF
				else
					begin
						if ( north_decryption_enable_state == 3'b000 && N_Ctrl_SOF )
							north_decryption_enable_state <= 3'b001;
						if ( north_decryption_enable_state == 3'b001 && NIor && rNI )
							north_decryption_enable_state <= 3'b010;
						if ( north_decryption_enable_state == 3'b010 && NIor && rNI )
							begin
								north_decryption_enable_state <= 3'b011;
								north_decryption_enable <= 1'b1;								
							end //end of if north_decryption_enable_state
					end //end of if else N_Ctrl_EOF
			end //end of else reset
	end //end always
	
//East Input Decryption Enable Detection
always @(posedge clock)
    begin
        if ( reset || !Ecsr[F] )
            begin
				east_decryption_enable_state <= 3'b000;
				east_decryption_enable <= 1'b0;
            end //end of if reset
		else
			begin
				if ( E_Ctrl_EOF )
					begin
						east_decryption_enable_state <= 3'b000;
						east_decryption_enable <= 1'b0;
					end //end of if E_Ctrl_EOF
				else
					begin
						if ( east_decryption_enable_state == 3'b000 && E_Ctrl_SOF )
							east_decryption_enable_state <= 3'b001;
						if ( east_decryption_enable_state == 3'b001 && EIor && rEI )
							east_decryption_enable_state <= 3'b010;
						if ( east_decryption_enable_state == 3'b010 && EIor && rEI )
							begin
								east_decryption_enable_state <= 3'b011;
								east_decryption_enable <= 1'b1;								
							end //end of if east_decryption_enable_state
					end //end of if else E_Ctrl_EOF
			end //end of else reset
	end //end always
	
//West Input Decryption Enable Detection
always @(posedge clock)
    begin
        if ( reset || !Wcsr[F] )
            begin
				west_decryption_enable_state <= 3'b000;
				west_decryption_enable <= 1'b0;
            end //end of if reset
		else
			begin
				if ( W_Ctrl_EOF )
					begin
						west_decryption_enable_state <= 3'b000;
						west_decryption_enable <= 1'b0;
					end //end of if W_Ctrl_EOF
				else
					begin
						if ( west_decryption_enable_state == 3'b000 && W_Ctrl_SOF )
							west_decryption_enable_state <= 3'b001;
						if ( west_decryption_enable_state == 3'b001 && WIor && rWI )
							west_decryption_enable_state <= 3'b010;
						if ( west_decryption_enable_state == 3'b010 && WIor && rWI )
							begin
								west_decryption_enable_state <= 3'b011;
								west_decryption_enable <= 1'b1;								
							end //end of if west_decryption_enable_state
					end //end of if else W_Ctrl_EOF
			end //end of else reset
	end //end always

//South Input Decryption Enable Detection
always @(posedge clock)
    begin
        if ( reset || !Scsr[F] )
            begin
				south_decryption_enable_state <= 3'b000;
				south_decryption_enable <= 1'b0;
            end //end of if reset
		else
			begin
				if ( S_Ctrl_EOF )
					begin
						south_decryption_enable_state <= 3'b000;
						south_decryption_enable <= 1'b0;
					end //end of if S_Ctrl_EOF
				else
					begin
						if ( south_decryption_enable_state == 3'b000 && S_Ctrl_SOF )
							south_decryption_enable_state <= 3'b001;
						if ( south_decryption_enable_state == 3'b001 && SIor && rSI )
							south_decryption_enable_state <= 3'b010;
						if ( south_decryption_enable_state == 3'b010 && SIor && rSI )
							begin
								south_decryption_enable_state <= 3'b011;
								south_decryption_enable <= 1'b1;								
							end //end of if south_decryption_enable_state
					end //end of if else S_Ctrl_EOF
			end //end of else reset
	end //end always
	

//End JLPL - 9_16_25

//JLPL - 9_17_25 -> Add Encryption Frame Detection to Enable Security Function

//North Input Encryption Enable Detection
always @(posedge clock)
    begin
        if ( reset || !Ncsr[8] )
            begin
				north_encryption_enable_state <= 3'b000;
				north_encryption_enable <= 1'b0;
            end //end of if reset
		else
			begin
				if ( north_output_pipe_in == 9'h1FD && N_output_pipe_write ) //Check for EOF Write
					begin
						north_encryption_enable_state <= 3'b000;
						north_encryption_enable <= 1'b0;
					end //end of if North EOF Control Write
				else
					begin
						if ( north_encryption_enable_state == 3'b000 && north_output_pipe_in == 9'h1FB && N_output_pipe_write ) //Check for SOF Write
							north_encryption_enable_state <= 3'b001;
						if ( north_encryption_enable_state == 3'b001 && north_output_pipe_in[8] == 1'b0 && N_output_pipe_write ) //Check for Data Write
							north_encryption_enable_state <= 3'b010;
						if ( north_encryption_enable_state == 3'b010 && north_output_pipe_in[8] == 1'b0 && N_output_pipe_write ) //Check for Data Write
							begin
								north_encryption_enable_state <= 3'b011;
								north_encryption_enable <= 1'b1;								
							end //end of if north_encryption_enable_state
					end //end of if else North EOF Control Write
			end //end of else reset
	end //end always

//East Input Encryption Enable Detection
always @(posedge clock)
    begin
        if ( reset || !Ecsr[8] )
            begin
				east_encryption_enable_state <= 3'b000;
				east_encryption_enable <= 1'b0;
            end //end of if reset
		else
			begin
				if ( east_output_pipe_in == 9'h1FD && E_output_pipe_write ) //Check for EOF Write
					begin
						east_encryption_enable_state <= 3'b000;
						east_encryption_enable <= 1'b0;
					end //end of if East EOF Control Write
				else
					begin
						if ( east_encryption_enable_state == 3'b000 && east_output_pipe_in == 9'h1FB && E_output_pipe_write ) //Check for SOF Write
							east_encryption_enable_state <= 3'b001;
						if ( east_encryption_enable_state == 3'b001 && east_output_pipe_in[8] == 1'b0 && E_output_pipe_write ) //Check for Data Write
							east_encryption_enable_state <= 3'b010;
						if ( east_encryption_enable_state == 3'b010 && east_output_pipe_in[8] == 1'b0 && E_output_pipe_write ) //Check for Data Write
							begin
								east_encryption_enable_state <= 3'b011;
								east_encryption_enable <= 1'b1;								
							end //end of if east_encryption_enable_state
					end //end of if else East EOF Control Write
			end //end of else reset
	end //end always

//West Input Encryption Enable Detection
always @(posedge clock)
    begin
        if ( reset || !Wcsr[8] )
            begin
				west_encryption_enable_state <= 3'b000;
				west_encryption_enable <= 1'b0;
            end //end of if reset
		else
			begin
				if ( west_output_pipe_in == 9'h1FD && W_output_pipe_write ) //Check for EOF Write
					begin
						west_encryption_enable_state <= 3'b000;
						west_encryption_enable <= 1'b0;
					end //end of if West EOF Control Write
				else
					begin
						if ( west_encryption_enable_state == 3'b000 && west_output_pipe_in == 9'h1FB && W_output_pipe_write ) //Check for SOF Write
							west_encryption_enable_state <= 3'b001;
						if ( west_encryption_enable_state == 3'b001 && west_output_pipe_in[8] == 1'b0 && W_output_pipe_write ) //Check for Data Write
							west_encryption_enable_state <= 3'b010;
						if ( west_encryption_enable_state == 3'b010 && west_output_pipe_in[8] == 1'b0 && W_output_pipe_write ) //Check for Data Write
							begin
								west_encryption_enable_state <= 3'b011;
								west_encryption_enable <= 1'b1;								
							end //end of if west_encryption_enable_state
					end //end of if else West EOF Control Write
			end //end of else reset
	end //end always

//South Input Encryption Enable Detection
always @(posedge clock)
    begin
        if ( reset || !Scsr[8] )
            begin
				south_encryption_enable_state <= 3'b000;
				south_encryption_enable <= 1'b0;
            end //end of if reset
		else
			begin
				if ( south_output_pipe_in == 9'h1FD && S_output_pipe_write ) //Check for EOF Write
					begin
						south_encryption_enable_state <= 3'b000;
						south_encryption_enable <= 1'b0;
					end //end of if South EOF Control Write
				else
					begin
						if ( south_encryption_enable_state == 3'b000 && south_output_pipe_in == 9'h1FB && S_output_pipe_write ) //Check for SOF Write
							south_encryption_enable_state <= 3'b001;
						if ( south_encryption_enable_state == 3'b001 && south_output_pipe_in[8] == 1'b0 && S_output_pipe_write ) //Check for Data Write
							south_encryption_enable_state <= 3'b010;
						if ( south_encryption_enable_state == 3'b010 && south_output_pipe_in[8] == 1'b0 && S_output_pipe_write ) //Check for Data Write
							begin
								south_encryption_enable_state <= 3'b011;
								south_encryption_enable <= 1'b1;								
							end //end of if south_encryption_enable_state
					end //end of if else South EOF Control Write
			end //end of else reset
	end //end always	

//End JLPL - 9_17_25


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
`ifdef   verify_NEWS

module t;

localparam  FALSE = 1'b0,
            TRUE  = 1'b1;
// mut inputs
reg          rNO,  rEO,  rWO,  rSO;      // Read
reg  [ 7:0]  iNI,  iEI,  iWI,  iSI;      // Data Inputs
reg          wNI,  wEI,  wWI,  wSI;      // Write
reg  [33:0]   cmiso, dmiso;              // For DMA reads from Memories
reg  [47:0]   imosi;                     // I/O interface from A2S
reg           sin;                       // UART serial input
reg           reset;
reg           clock;

// mut outputs
wire [ 7:0]   NOo,  EOo,  WOo,  SOo;     // Data Outputs
wire          NOor, EOor, WOor, SOor;    // Output ready
wire          NIir, EIir, WIir, SIir;    // input ready
wire [65:0]   lmosi;                     // For DMA write to Memories
wire [33:0]   imiso;                     // I/O interface to A2S
wire          sout;                      // UART serial output

NEWS mut (
   // Local and NEWS FIFO Output Interfaces
     .NOo(NOo),   .EOo(EOo),   .WOo(WOo),   .SOo(SOo),         // Data Outputs
    .NOor(NOor), .EOor(EOor), .WOor(WOor), .SOor(SOor),     // Output ready
    .rNO(rNO),   .rEO(rEO),   .rWO(rWO),   .rSO(rSO),       // Read
   // Local and NEWS FIFO Input  Interfaces
     .iNI(iNI),   .iEI(iEI),   .iWI(iWI),   .iSI(iSI),      // Data Inputs
     .wNI(wNI),   .wEI(wEI),   .wWI(wWI),   .wSI(wSI),      // Write
     .NIir(NIir), .EIir(EIir), .WIir(WIir), .SIir(SIir),    // input ready
   // Memory Interface
   .lmosi(lmosi),                                           // For DMA write to Memories
   .cmiso(cmiso), .dmiso(dmiso),                            // For DMA reads from Memories
   // A2S Interface
   .imiso(imiso),                                           // I/O interface to A2S
   .imosi(imosi),                                           // I/O interface from A2S
   // Global
   .sout(sout),                                             // UART serial output
   .sin(sin),                                               // UART serial input
   .reset(reset),
   .clock(clock)
   );

initial begin
   clock = 0;
   forever
      clock = #10 ~clock;
   end

initial begin
   iNI   = 0;
   iEI   = 0;
   iWI   = 0;
   iSI   = 0;
   rNO   = FALSE;
   rEO   = FALSE;
   rWO   = FALSE;
   rSO   = FALSE;
   wNI   = FALSE;
   wEI   = FALSE;
   wWI   = FALSE;
   wSI   = FALSE;
   cmiso = 0;
   dmiso = 0;
   imosi = 0;
   sin   = FALSE;
   reset = 0;
    repeat (100) @(posedge clock) #1;
   $finish;
end

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
