/*==============================================================================================================================

    Copyright © 2022, 2023 Advanced Architectures

    All rights reserved
    Confidential Information
    Limited Distribution to Authorized Persons Only
    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

    Project Name         : A2_NEWS_CoP

    Description          : Application Layer TileZero NEWS interface Gasket
                           

================================================================================================================================
    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
    Notes for Use and Synthesis

    Uses NEWS System Verilog Code Structure referenced in the module call to news

===============================================================================================================================*/

`ifdef  synthesis //JLSW
   `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
`else //JL
   `include "A2_project_settings.vh"
`endif //JL

//  NEWS Higher Newport_Core Level Pipeline
//**************************************************************************************************
//  'i'  cardinal (NEWS)   -------- Example for depth == 4  --------
//  'j'  pipe stage num
//        j=0            j=1             j=2             j=3            j=4
//  NEWS        PIPE-0          PIPE-1          PIPE-2          PIPE-3     \  DFE
//             +-----+         +-----+         +-----+         +-----+      \
//  =tx_do[0]=>|di do|===[1]==>|di do|===[2]==>|di do|===[3]==>|di do|=tx_do[4]=>   TxDataIn
//             |     |         |     |         |     |         |     |
//     --give->|wr or|--give-->|wr or|--give-->|wr or|--give-->|wr or|--give->      write
//             |     |         |     |         |     |         |     |
//     <-take--|ir rd|<-take---|ir rd|<-take---|ir rd|<-take---|ir rd|<-take--      input_ready
//             +--^--+         +--^--+         +--^--+         +--^--+
//      clock ----|------>>-------|------->>------|------>>-------|------+---->
//                                                                       |
//                                                                       |
//  NEWS        PIPE-0          PIPE-1          PIPE-2          PIPE-3   |    DFE
//             +-----+         +-----+         +-----+         +-----+   |
//  <=rx_di[0]=|do di|<==[1]===|do di|<==[2]===|do di|<==[3]===|do di|<=rx_di[4]=   DataOut
//             |     |         |     |         |     |         |     |   |
//     <-give--|or wr|<--give--|or wr|<--give--|or wr|<--give--|or wr|<--give--     output_ready
//             |     |         |     |         |     |         |     |   |
//     --take->|rd ir|--take-->|rd ir|--take-->|rd ir|--take-->|rd ir|--take-->     read
//             +--^--+         +--^--+         +--^--+         +--^--+   |
//                |------<<-------|------<<-------|------<<-------|---<<-+
//***************************************************************************************************

module   A2_NEWS_CoP #(
   parameter   [12:0]   BASE = 13'h00000
   )(
   input  wire [47:0]   imosi,   // Slave data in  {cIO[2:0] aIO[12:0], iIO[31:0]}
   output wire [32:0]   imiso,   // Slave data out {               IOk, IOo[31:0]}
   output wire          intpt,   // NEWS status interrupt
   output wire          intpt_ldma,    // JLPL - 9_6_25 - DMA Outgoing Transfer Complete interrupt
   output wire [ 4:0]   nm_lwr, //JLPL test //JLPL test //JLPL - 8_28_25 - Have to leave in because of compiler problem assigning proper signals to nmosi output
   output wire [ 3:0]   nm_uppr, //JLPL test //JLPL test //JLPL - 8_28_25 - Have to leave in because of compiler problem assigning proper signals to nmosi output
   output wire [ 8:0]   nmosi,                                             // NEWS Output Pipeline Outputs //***
   output wire [ 8:0]   emosi,                                             // NEWS Output Pipeline Outputs //***
   output wire [ 8:0]   wmosi,                                             // NEWS Output Pipeline Outputs //***
   output wire [ 8:0]   smosi,                                             // NEWS Output Pipeline Outputs //***
   output wire          nmosi_give,                                        // NEWS Output Pipeline give //***
   output wire          emosi_give,                                        // NEWS Output Pipeline give //*** 
   output wire          wmosi_give,                                        // NEWS Output Pipeline give //***
   output wire          smosi_give,                                        // NEWS Output Pipeline give //***
   input  wire          nmosi_take,                                        // NEWS Input Pipeline take //***
   input  wire          emosi_take,                                        // NEWS Input Pipeline take //***
   input  wire          wmosi_take,                                        // NEWS Input Pipeline take //***
   input  wire          smosi_take,                                        // NEWS Input Pipeline take //***
   output wire          nmosi_clock,                                       // NEWS Output Pipeline Clock //***
   output wire          emosi_clock,                                       // NEWS Output Pipeline Clock //***
   output wire          wmosi_clock,                                       // NEWS Output Pipeline Clock //***
   output wire          smosi_clock,                                       // NEWS Output Pipeline Clock //***
   output wire          nmosi_reset,                                       // NEWS Output Pipeline Reset //***
   output wire          emosi_reset,                                       // NEWS Output Pipeline Reset //***
   output wire          wmosi_reset,                                       // NEWS Output Pipeline Reset //***
   output wire          smosi_reset,                                       // NEWS Output Pipeline Reset //***
   input  wire [ 8:0]   nmiso,                                             // NEWS Input  Pipeline Inputs //***
   input  wire [ 8:0]   emiso,                                             // NEWS Input  Pipeline Inputs //***
   input  wire [ 8:0]   wmiso,                                             // NEWS Input  Pipeline Inputs //***
   input  wire [ 8:0]   smiso,                                             // NEWS Input  Pipeline Inputs //***
   input  wire          nmiso_give,                                        // NEWS Input  Pipeline give //***
   input  wire          emiso_give,                                        // NEWS Input  Pipeline give //***
   input  wire          wmiso_give,                                        // NEWS Input  Pipeline give //***
   input  wire          smiso_give,                                        // NEWS Input  Pipeline give //***
   output wire          nmiso_take,                                        // NEWS Output  Pipeline take //***
   output wire          emiso_take,                                        // NEWS Output  Pipeline take //***
   output wire          wmiso_take,                                        // NEWS Output  Pipeline take //***
   output wire          smiso_take,                                        // NEWS Output  Pipeline take //***
   input  wire          nmiso_clock,                                       // NEWS Input  Pipeline Clock //JLSW
   input  wire          emiso_clock,                                       // NEWS Input  Pipeline Clock //JLSW
   input  wire          wmiso_clock,                                       // NEWS Input  Pipeline Clock //JLSW
   input  wire          smiso_clock,                                       // NEWS Input  Pipeline Clock //JLSW
   input  wire          nmiso_reset,                                       // NEWS Input  Pipeline Reset //JLSW
   input  wire          emiso_reset,                                       // NEWS Input  Pipeline Reset //JLSW
   input  wire          wmiso_reset,                                       // NEWS Input  Pipeline Reset //JLSW
   input  wire          smiso_reset,                                       // NEWS Input  Pipeline Reset //JLSW
   output wire [4:0]    rmosi,                                             // NEWS Output Requests to AES
   input  wire          rmosi_fb,                                          // NEWS Input Request Ready from AES //***
   input  wire [40:0]   rmiso,                                             // NEWS Input Encryption Data from AES //JLSW
   output wire          rmiso_fb,                                          // NEWS Input Encryption Data Read to AES //***
   output wire          parity_error, //JLPL - 11_8_25
   // Memory Interface                 // Memory is a slave
   output wire [66:0]   lmosi,                                             // 67 = {lreq, lwr, lrd, lwrd[31:0], ladr[31:0]} //JLSW -> To RAM_3PX DMA Port in TileZero //JLPL - 8_27_25
   input  wire [33:0]   lmiso,                                             //      {lack, lgnt, lrdd[31:0]}  = 34 //JLSW       -> From RAM_3PX DMA Port in TileZero
   output wire          dummy_output,                                      //JLSW - temporary
   input                break_input,   //***
   input                reset,   //Active High Reset Input
   input                clock    //TRNG Interface Clock
   );


// DMA Pipes ======================================================================================================================== //JLSW - temporary

//wire LOir;
//wire [31:0] LOdo;
//wire LOor;
//wire LIir_pipe;
//wire LOrd = LIir_pipe;
//wire [63:0] LIdi = {LOdo[31:0],LOdo[31:0]};
//wire LIwr = LOor;
//wire [63:0] LIdo;
//wire LIor;
//wire LIrd = 1'b1;

//assign lmosi = {LIor,LIor,LIdo};

//A2_pipe #(64) i_LI (    // Local input from NEWS switch => Writes to Memory
   // Pipe Output to RAM
//   .pdo  (LIdo),
//   .por  (LIor),        // Creates write access
//   .prd  (LIrd),        // POPs write data on grant
   // Pipe Input
//   .pdi  (LIdi),
//   .pir  (LIir_pipe),
//   .pwr  (LIwr),
//   .clock(clock),
//   .reset(reset)
//   );

//A2_pipe #(32) i_LO (    // Reads from Memory => Local output to NEWS switch
   // Pipe Output
//   .pdo  (LOdo),
//   .por  (LOor),
//   .prd  (LOrd),
   // Pipe Input to RAM
//   .pdi  (lmiso[31:0]),
//   .pir  (LOir),        // Creates read access
//   .pwr  (lmiso[32]),        // Pushes new data on acknowledge
//   .clock(clock),
//   .reset(reset));

//JLSW - End of temporary

//===============================================================================================================================
// CLOCKS & RESETS SYNCRONISER ==================================================================================================
//===============================================================================================================================

wire sync_reset = reset; //JLPL - 8_1_25 -> reset is synchronous coming in so remove extra synchronization

//A2_reset rst_clock    (.q(sync_reset),   .reset(reset), .clock(clock    ));   // Synchronized to Raceway Logic Input Clock //JLPL - 8_1_25

// NEWS Interfaces Decollated
 
   wire NOor,EOor,WOor,SOor;
   wire [8:0] NOo,EOo,WOo,SOo;
   wire NIir,EIir,WIir,SIir;
   wire wNI,wEI,wWI,wSI,wLI; //JLPL - 8_28_25 Declare wLI
   wire rNO,rEO,rWO,rSO;
   wire [8:0] iNI,iEI,iWI,iSI;
   //wire rmosi_wr;  //JLPL - 8_28_25 Comment Out
   //wire rmosi_ir;  //JLPL - 8_28_25 Comment Out
   wire [10:0] cc1;
	
   reg flag = 1'b0;
   reg LIir = 1'b1;
   //wire rLO = 1'b0; //JLPL - 11_3_25
   wire rLO; //JLPL - 11_3_25  - Remove multiple driver 

//***

   assign nm_lwr = NOo[4:0]; //JLPL test //JLPL - 8_28_25 - Have to leave in because of compiler problem assigning proper signals to nmosi output
   assign nm_uppr = NOo[8:5]; //JLPL test //JLPL - 8_28_25 - Have to leave in because of compiler problem assigning proper signals to nmosi output
   assign nmosi = NOo; //***
   assign emosi = EOo; //***
   assign wmosi = WOo; //***
   assign smosi = SOo; //***
   assign nmosi_give = rNO; //JLPL
   assign emosi_give = rEO; //JLPL
   assign wmosi_give = rWO; //JLPL
   assign smosi_give = rSO; //JLPL
   assign NOor = nmosi_take;  //JLPL
   assign EOor = emosi_take;  //JLPL
   assign WOor = wmosi_take;  //JLPL
   assign SOor = smosi_take;  //JLPL
   assign nmosi_clock = clock; //***
   assign emosi_clock = clock; //***
   assign wmosi_clock = clock; //***
   assign smosi_clock = clock; //***
   assign nmosi_reset = sync_reset; //*** This a synchronous reset out
   assign emosi_reset = sync_reset; //*** This a synchronous reset out
   assign wmosi_reset = sync_reset; //*** This a synchronous reset out
   assign smosi_reset = sync_reset; //*** This a synchronous reset out
		
   assign iNI = nmiso; //***
   assign iEI = emiso; //***
   assign iWI = wmiso; //***
   assign iSI = smiso; //***
   assign NIir = nmiso_give; //JLPL
   assign EIir = emiso_give; //JLPL
   assign WIir = wmiso_give; //JLPL
   assign SIir = smiso_give; //JLPL
   assign nmiso_take = wNI; //JLPL
   assign emiso_take = wEI; //JLPL
   assign wmiso_take = wWI; //JLPL
   assign smiso_take = wSI; //JLPL

assign dummy_output = nmiso_clock & emiso_clock & wmiso_clock & smiso_clock & nmiso_reset & emiso_reset & wmiso_reset & smiso_reset; //JLSW - temporary
 
// NEWS Top Module Instantiation -------------------------------------------------------------------------------------------------------------------------

// I/O Module Number of Registers

//`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/NEWPORT_IO_addresses.vh" - Put back in later after finding ncverilog compile sensitivity with local param header NEWPORT_IO_addresses.vh

wire [7:0] iLI = 8'h55; //JL
//assign nmosi[4] = 1'b0; //JLSW - TBD from Roger //JLPL - 11_3_25 - Remove multiple driver
assign rmosi[3] = 1'b0; //JLPL - 4 bit addressing in AES so upper bit of address is always 0 for NEWS - 7_29_25
   NEWS #(            //JLPL - 8_18_25
      .BASE  (BASE)   //JLPL - 8_18_25
   ) news_inst  //JLPL - 8_18_25
      // NEWS FIFO Output Interfaces
   (.NOo                       (NOo),  // Data Output
    .EOo                       (EOo),  // Data Output
    .WOo                       (WOo),  // Data Output
    .SOo                       (SOo),  // Data Output
    //.LOo                       (LOo),  // Data Output //Joel 12_18_24 - Remove
    .NOor                      (NOor), // Data Output
    .EOor                      (EOor), // Data Output
    .WOor                      (WOor), // Data Output
    .SOor                      (SOor), // Data Output
    //.LOor                      (LOor), // Data Output //Joel 12_18_24 - Remove
    .rNO                       (rNO),  // Read
    .rEO                       (rEO),  // Read
    .rWO                       (rWO),  // Read
    .rSO                       (rSO),  // Read
    .rLO                       (rLO),  // Read
    // NEWS FIFO Input  Interfaces
    .iNI                       (iNI), // Data Input //Joel 11_6_24 - Change from iNI //Joel 11_7_24 - Latest change from left_dfe_rcv_data_out
    .iEI                       (iEI), // Data Input
    .iWI                       (iWI), // Data Input
    .iSI                       (iSI), // Data Input
    .iLI                       (iLI), // Data Input
    .wNI                       (wNI), // Write
    .wEI                       (wEI), // Write
    .wWI                       (wWI), // Write
    .wSI                       (wSI), // Write
    .wLI                       (wLI), // Write
    .NIir                      (NIir), // input ready //Joel 11_6_24 - Change from NIir //Joel 11_7_24 - Latest Change from left_dfe_rcv_data_ready
    .EIir                      (EIir), // input ready
    .WIir                      (WIir), // input ready
    .SIir                      (SIir), // input ready
    .LIir                      (LIir), // input ready
    // A2S Interface
    .imiso                     (imiso),
    .imosi                     (imosi),
    // AES Encryption Block Input Interface
    .amiso                     (rmiso),      // AES output   41 {CBor, ECBad[3:0], ECBpo[3:0], ECB[31:0]} NB ir belongs to nmosi //Joel 2_14_25 //***
    .amiso_rd                  (rmiso_fb),   // Read / Take  (Selected destination is ready!) //Joel 2_14_25 //*** //JLPL - 7_30_25 was rmiso_rd
    // AES Request ECB Interface
    .rmosi                     (rmosi[2:0]),      // AES requests //Joel 2_14_25
    .rmosi_wr                  (rmosi[4]),   //Joel 2_14_25 //JLPL - Move to proper rmosi array index 4 - 7_29_25
    .rmosi_ir                  (rmosi_fb),   //Joel 2_14_25
    .parity_error              (parity_error), //JLPL - 11_8_25
	//----------- Memory Interface --------------------// Memory is a slave  //JLPL - 8_27_25
	.lmosi                     (lmosi), //JLPL - 8_27_25
	.lmiso                     (lmiso), //JLPL - 8_27_25
   // Interrupt                        //JLPL - 9_6_25
      .intpt_ldma   (intpt_ldma),      //JLPL - 9_6_25
    //------------------- GLOBAL   --------------------//
    .cc                        (cc1), //Joel 11_11_24
    .intpt                     (intpt), //JLPL
    .reset                     (sync_reset), //***
    .clock                     (clock));

 
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