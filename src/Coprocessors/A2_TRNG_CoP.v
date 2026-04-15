/*==============================================================================================================================

    Copyright © 2022, 2023 Advanced Architectures

    All rights reserved
    Confidential Information
    Limited Distribution to Authorized Persons Only
    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

    Project Name         : A2_TRNG_CoP

    Description          : Application Layer TileOne TRNG interface Gasket
                           

================================================================================================================================
    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
    Notes for Use and Synthesis

    Uses Xiphera System Verilog Code Structure referenced in the module call to xip8001b

===============================================================================================================================*/

`define NUM_REGS                      7  //Hard Coded Number of TRNG Registers //JL

`ifndef  RELATIVE_FILENAMES //JL
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh" //JL
`else //JL
   `include "A2_project_settings.vh" //JL
`endif //JL

module   A2_TRNG_CoP #(
   parameter   [12:0]   BASE = 13'h00000
   )(
   input  wire [47:0]   imosi,   // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
   output wire [32:0]   imiso,   // Slave data out {               IOk, IOo[31:0]}
   output wire          intpt_trng,      // TRNG data ready interrupt
   input                reset,   //Active High Reset Input
   input                clock    //TRNG Interface Clock
   );

//Note Control Registers needs to be at Gasket Level and then fed into Xiphera Top Level call
 
// TRNG Xiphera Top Module Instantiation -------------------------------------------------------------------------------------------------------------------------

// I/O Module Number of Registers

localparam  LG2REGS  =  `LG2(`NUM_REGS);
//JL - Use Romeo convention for Simulation vs Synthesis Include Configurations
`ifdef synthesis
	 `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_IO_addresses.vh" //Comment out until Romeo ready for synthesis of compilable Top Netlist
	 //`include "/proj/TekStart/lokotech/soc/users/jfechter/cognitum_a0/src/include/COGNITUM_IO_addresses.vh" //Use local repo copy until Romeo's directory is updated to repo tip
`else
	`include "COGNITUM_IO_addresses.vh"
`endif
//End JL

// imosi\imiso Interface decoding ===============================================================================================

wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
// Decollate imosi
assign   {cIO, iIO, aIO} = imosi;

wire                     in_range =  aIO[12:LG2REGS]  == BASE[12:LG2REGS];
wire  [`NUM_REGS-1:0]     write =  ((cIO==3'b110) & in_range)  << aIO[LG2REGS-1:0];
wire  [`NUM_REGS-1:0]      read =  ((cIO==3'b101) & in_range)  << aIO[LG2REGS-1:0];

//Flip/Flops
reg [31:0] trng_ctrl = 32'h00000001;
reg [31:0] trng_health = 32'h0;
reg [31:0] trng_status = 32'h0;
reg [31:0] trng_random = 32'h0;
reg [31:0] trng_mwaddr = 32'h0;
reg [31:0] trng_sample_freq = 32'h0;
reg [31:0] trng_sample_div = 32'h0;

//Signals

    // Status outputs
wire 		zeroised_o;
wire    	startup_concluded_o;
wire		startup_success_o;

wire		healthy_o;
wire [1:0]  ht_o; // adaptive proportion bit position 1, rep count bit position 0,
        
    // Main data interface
wire [31:0] data_o; //Was Parameterized - FIFO_WIDTH
reg 		read_i = 1'b0;
wire		empty_o;

    // Bypass modes
 reg		bypass_cbc_i = 1'b0;
 reg		bypass_ht_i = 1'b0;

    // Health test parameters
 reg [1:0] 	window_size_i = 2'b00; // 0=1024, 1=2048, 2=4096
 reg [11:0] ap_cutoff_i = 12'h400;  // possible values 512-4096
 reg [5:0] 	rep_count_limit_i = 6'h20;   // possible values 0-63

    // entropy source ctrl and info
// reg [30:0] [1:0] disable_osc_i; //Was parameterized OSC_DEPTH //JL
 reg [30:0] disable_osc_i [1:0]; //Was parameterized OSC_DEPTH //JL
 reg [8:0]		  freq_div_i = 9'h100; // possible values 0-512
 wire [15:0] 	  trng_clk_counter_o; 
 
 wire trng_reset;
 wire en_i;

//End Signals


always @(*)
    begin
       disable_osc_i[1] = 31'h0; //Enable all oscillators
	   disable_osc_i[0] = 31'h0; //Enable all oscillators
    end //end of always

//TRNG Registers Write Logic	
always @(posedge clock) if (reset)   trng_ctrl  <= #1 32'h0; 
   else if ( write[TRNG_CTRL])       trng_ctrl  <= #1 iIO; 
   
always @(posedge clock) if (reset)   trng_health  <= #1 32'h0; 
   else if ( write[TRNG_HEALTH])     trng_health  <= #1 iIO; 
   
always @(posedge clock) if (reset)   trng_status  <= #1 32'h0; 
   else if ( write[TRNG_STATUS])     trng_status  <= #1 iIO; 
   
always @(posedge clock) if (reset)   trng_random  <= #1 32'h0; 
   else if ( write[TRNG_RANDOM])     trng_random  <= #1 iIO;
   
always @(posedge clock) if (reset)   trng_mwaddr  <= #1 32'h0; 
   else if ( write[TRNG_MWADDR])     trng_mwaddr  <= #1 iIO;
   
always @(posedge clock) if (reset)   trng_sample_freq  <= #1 32'h0; 
   else if ( write[TRNG_SAMPLE_FREQ])     trng_sample_freq  <= #1 iIO;
   
always @(posedge clock) if (reset)   trng_sample_div  <= #1 32'h0; 
   else if ( write[TRNG_SAMPLE_DIV])     trng_sample_div  <= #1 iIO;

//TRNG Reads
assign IOo[31:0] = read[TRNG_CTRL] == 1 ? trng_ctrl :
                         read[TRNG_HEALTH] == 1 ? trng_health :
                         read[TRNG_STATUS] == 1 ? trng_status :
                         read[TRNG_RANDOM] == 1 ? trng_random :
                         read[TRNG_MWADDR] == 1 ? trng_mwaddr :
                         read[TRNG_SAMPLE_FREQ] == 1 ? trng_sample_freq :
                         read[TRNG_SAMPLE_DIV] == 1 ? trng_sample_div :                                                
                         32'h0;

assign  IOk  =  |write[`NUM_REGS-1:0] |  |read[`NUM_REGS-1:0];
assign  imiso =  {IOk, IOo};

assign trng_reset = reset | trng_ctrl[0];
assign en_i = trng_ctrl[1];

/* JL - Comment out to remove IP

xip8001b #(
    .OSC_DEPTH(31),
    .OSC_WIDTH(11),
    .CLK_OSC_WIDTH(301)  //JL
)  i_TRNG_Xiphera (
   // Control Interface
    .clki  	    (clock),
    .rst_i		(trng_reset),
    .en_i     	(en_i),
	
	//Register interface
	.trng_ctrl(trng_ctrl),
	.trng_health(trng_health),
	.trng_status(trng_status),
	.trng_random(trng_random),
	.trng_mwaddr(trng_mwaddr),
	.trng_sample_freq(trng_sample_freq),
	.trng_sample_div(trng_sample_div),

    // Status outputs
    .zeroised_o	(zeroised_o),
    .startup_concluded_o	(startup_concluded_o),
    .startup_success_o	(startup_success_o),

    .healthy_o	(healthy_o),
    .ht_o		(ht_o),
 
    // A2S Interface
    .imiso(imiso),            // I/O interface to A2S Tile0
    .imosi(imosi),            // I/O interface from A2S Tile0
	
    // Main data interface
    .data_o		(data_o),
    .read_i		(read_i),
    .empty_o	(empty_o),

    // Bypass modes
    .bypass_cbc_i	(bypass_cbc_i),
    .bypass_ht_i	(bypass_ht_i),

    // Health test parameters
    .window_size_i	(window_size_i), // 0=1024, 1=2048, 2=4096
    .ap_cutoff_i	(ap_cutoff_i),  // possible values 512-4096
    .rep_count_limit_i	(rep_count_limit_i),   // possible values 0-63

    // entropy source ctrl and info
    .disable_osc_i	(disable_osc_i[0]), //JL
    .freq_div_i		(freq_div_i), // possible values 0-512
    .trng_clk_counter_o	(trng_clk_counter_o) 
   ); 
*/  //JL - End of Comment Out
 
endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
`ifdef  verify_tile
`endif
